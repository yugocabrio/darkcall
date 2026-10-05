#!/usr/bin/env python3
"""Committed REP3 bids -> masked refund delivery to operators.

Never reconstructs maximum bids. Fresh private field masks and opened refunds
remain under ignored .local. No native funding or receipt authentication here.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import signal
import socket
import subprocess
import time
from mpc_process import separate_session, signal_child

ROOT = pathlib.Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / '.deps/co-snarks'
PIN = '467a2abbad940d40ed78f38d424cc21920988976'
BIN = UPSTREAM / 'target-aes-native/release/co-noir'
NARGO = ROOT / '.deps/noir-1.0.0-rc.2/nargo'
INPUT = ROOT / 'target/release/darkcall-input-tool'
CLIENT = ROOT / 'circuits/bid_input/target/darkcall_bid_input.json'
CRS = UPSTREAM / 'co-noir/co-noir-common/src/crs'


def main():
    # coNoir writes secret input/witness shares itself. Restrict its inherited
    # creation mode as well as the enclosing directory, independent of shell.
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--book-run', type=pathlib.Path, required=True)
    parser.add_argument('--result-file', type=pathlib.Path)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument('--winner-flags', action='store_true', help='Separate blinded-commitment circuit includes privately masked winner bits')
    modes.add_argument('--legacy-masks', action='store_true', help='Historical v1 only; public candidate guessing reveals refunds')
    args = parser.parse_args()
    circuit_name = 'delivery_flags4' if args.winner_flags else 'delivery4' if args.legacy_masks else 'delivery_private4'
    circuit_path = ROOT / f'circuits/{circuit_name}/target/darkcall_{circuit_name}.json'
    hash_rule = ('fresh-field-mask-refund-and-winner-delivery-v2' if args.winner_flags else
                 'fresh-field-mask-refund-delivery-v1' if args.legacy_masks else 'fresh-field-mask-refund-delivery-v2')
    book = args.book_run.resolve()
    if args.result_file is not None and not args.result_file.resolve().is_relative_to(ROOT / '.local'):
        raise RuntimeError('Result file must remain in ignored local fixtures')
    if any(not p.is_relative_to(ROOT / '.local') for p in [book]):
        raise RuntimeError('Only ignored synthetic local runs are supported')
    if subprocess.check_output(['git', '-C', str(UPSTREAM), 'rev-parse', 'HEAD'], text=True).strip() != PIN:
        raise RuntimeError('Wrong coNoir source pin')
    if 'nargo version = 1.0.0-rc.2\n' not in subprocess.check_output([str(NARGO), '--version'], text=True):
        raise RuntimeError('Wrong Noir version')
    for circuit in [circuit_name, 'bid_input']:
        subprocess.run([str(NARGO), 'compile', '--program-dir', str(ROOT / 'circuits' / circuit)], check=True)
    run = ROOT / '.local' / ('mpc-' + circuit_name + '-' + time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()))
    run.mkdir(mode=0o700)
    summary = dict(scope='operator-only masked refund delivery', hash_rule=hash_rule, circuit=circuit_name, source_pin=PIN,
                   independent_hosts=False, funded_transaction=False, custody_signatures=False,
                   input_proofs_verified=False, masked_refunds_delivered=False,
                   binary_sha256=hashlib.sha256(BIN.read_bytes()).hexdigest(), stages=[])
    env = {**os.environ, 'RUST_LOG':'info', 'RAYON_NUM_THREADS':'2'}

    def record(name, commands, timeout=600, binary=BIN):
        print(name, 'started', flush=True)
        start = time.monotonic()
        stage = dict(name=name)
        children, handles, logs = [], [], []
        try:
            for i, command in enumerate(commands):
                log = run / f'{name}-{i}.log'
                logs.append(log)
                handles.append(log.open('wb'))
                children.append(subprocess.Popen(['/usr/bin/time', '-l', str(binary), *map(str,command)],
                    cwd=run, env=env, stdout=handles[-1], stderr=subprocess.STDOUT, start_new_session=separate_session()))
            while any(p.poll() is None for p in children):
                if any(p.poll() not in (None, 0) for p in children):
                    raise RuntimeError(name + ' failed; inspect ignored logs')
                if time.monotonic() - start > timeout:
                    raise TimeoutError(name + ' exceeded bounded timeout')
                time.sleep(.2)
            if any(p.wait() for p in children):
                raise RuntimeError(name + ' failed; inspect ignored logs')
            stage['passed'] = True
            print(name, 'passed', flush=True)
        except BaseException as error:
            stage.update(passed=False,error=str(error))
            raise
        finally:
            for p in children:
                if p.poll() is None: signal_child(p,signal.SIGTERM)
            for p in children:
                try: p.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    signal_child(p,signal.SIGKILL)
                    p.wait()
            for h in handles: h.close()
            stage['seconds'] = round(time.monotonic()-start,3)
            stage['peak_rss_bytes_per_process'] = []
            for log in logs:
                found = re.findall(r'(\d+)\s+maximum resident set size',log.read_text(errors='replace'))
                stage['peak_rss_bytes_per_process'].append(int(found[-1]) if found else None)
            summary['stages'].append(stage)
            (run/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')

    fixture_mode = 'flags-fixture' if args.winner_flags else 'delivery-fixture' if args.legacy_masks else 'private-fixture'
    record('delivery-fixture', [[fixture_mode,book/'book.json',run/'plan']], binary=INPUT)
    record('trusted-client-vk', [['create-vk','--circuit',CLIENT,'--crs',CRS/'bn254_g1.dat','--hasher','poseidon2','--vk',run/'client-vk']])
    record('client-proof-verification', [['verify','--proof',book/f'client{i}/proof_plaindriver',
        '--public-input',book/f'client{i}/public_inputs_plaindriver','--vk',run/'client-vk',
        '--crs',CRS/'bn254_g2.dat','--hasher','poseidon2','--has-zk'] for i in range(4)])
    record('share-checks', [['check',book/'book.json',i,p,book/f'bidder{i}.toml.{p}.shared',
        book/f'client{i}/public_inputs_plaindriver'] for i in range(4) for p in range(3)], binary=INPUT)
    summary.update(input_proofs_verified=True,validated_share_pairs=12)
    for name in ['public','operator']:
        record('split-'+name,[['split-input','--circuit',circuit_path,'--input',run/f'plan/{name}.toml','--protocol','REP3','--out-dir',run]])
    sockets, ports = [], []
    try:
        for _ in range(3):
            sock=socket.socket(); sock.bind(('127.0.0.1',0)); sockets.append(sock); ports.append(sock.getsockname()[1])
        data=UPSTREAM/'co-noir/co-noir/examples/data'
        for party in range(3):
            config=f'[network]\nmy_id = {party}\nbind_addr = "127.0.0.1:{ports[party]}"\nmax_frame_length = 469762056\n'
            for peer,port in enumerate(ports): config+=f'\n[[network.parties]]\nid = {peer}\ndns_name = "127.0.0.1:{port}"\n'
            config+='\n[network.tls]\nkey = '+json.dumps(str(data/f'key{party}.der'))+'\ncerts = '+json.dumps([str(data/f'cert{i}.der') for i in range(3)])+'\n'
            (run/f'party{party}.toml').write_text(config)
            inputs=[]
            for i in range(4): inputs += ['--inputs',book/f'bidder{i}.toml.{party}.shared']
            for name in ['public','operator']: inputs += ['--inputs',run/f'{name}.toml.{party}.shared']
            record(f'merge-{party}',[['merge-input-shares','--circuit',circuit_path,'--protocol','REP3',*inputs,'--out',run/f'input.{party}.shared']])
    finally:
        for sock in sockets: sock.close()
    record('witness',[['generate-witness','--circuit',circuit_path,'--protocol','REP3','--input',run/f'input.{i}.shared',
        '--config',run/f'party{i}.toml','--out',run/f'witness.{i}.shared'] for i in range(3)],1800)
    record('vk',[['create-vk','--circuit',circuit_path,'--crs',CRS/'bn254_g1.dat','--hasher','poseidon2','--vk',run/'vk']])
    record('proof',[['build-and-generate-proof','--circuit',circuit_path,'--protocol','REP3','--witness',run/f'witness.{i}.shared',
        '--config',run/f'party{i}.toml','--crs',CRS/'bn254_g1.dat','--hasher','poseidon2','--vk',run/'vk',
        '--zk','--out',run/f'proof.{i}','--public-input',run/f'public-input{i}'] for i in range(3)],7200)
    verify=['verify','--proof',run/'proof.0','--vk',run/'vk','--crs',CRS/'bn254_g2.dat','--hasher','poseidon2','--has-zk']
    record('verify',[[*verify,'--public-input',run/'public-input0']])
    encoded=(run/'public-input0').read_bytes()
    if any((run/f'public-input{i}').read_bytes()!=encoded for i in [1,2]): raise RuntimeError('MPC parties disagree')
    fields=[int.from_bytes(encoded[i:i+32],'big') for i in range(0,len(encoded),32)]
    if len(fields)!=(187 if args.winner_flags else 179): raise RuntimeError('Unexpected delivery statement size')
    result = ['flags-result',book/'book.json',run/'plan/masks.json',run/'plan/winner-masks.json',run/'public-input0',run/'delivered-amounts.json'] if args.winner_flags else ['delivery-result' if args.legacy_masks else 'private-result',book/'book.json',run/'plan/masks.json',run/'public-input0',run/'delivered-amounts.json']
    record('deliver-refunds',[result],binary=INPUT)
    offset = 4 if args.winner_flags else 0
    changed_fields = [('price',173+offset),('root',168),('cipher',175+offset)]
    if args.winner_flags: changed_fields.append(('winner',183))
    for name,index in changed_fields:
        changed=bytearray(encoded); changed[index*32:(index+1)*32]=(fields[index]^1).to_bytes(32,'big')
        (run/f'tampered-{name}').write_bytes(changed)
        log=run/f'reject-{name}.log'
        with log.open('wb') as out:
            result=subprocess.run([str(BIN),*map(str,[*verify,'--public-input',run/f'tampered-{name}'])],stdout=out,stderr=subprocess.STDOUT,env=env,timeout=300)
        if result.returncode==0 or 'Proof verification failed' not in log.read_text(): raise RuntimeError('No cryptographic rejection of '+name)
    summary.update(passed=True,price=fields[173+offset],sold=fields[174+offset],masked_refunds_delivered=True,maximum_bids_reconstructed=False,
        masked_winner_flags_delivered=args.winner_flags,changed_masked_winner_rejected=args.winner_flags,
        mask_commitments_blinded=not args.legacy_masks,legacy_candidate_guessing_leak=args.legacy_masks,
        proof_bytes=(run/'proof.0').stat().st_size,changed_price_root_cipher_rejected=True,
        external_barretenberg_verified=False,funding_authenticated=False,receiver_receipts_authenticated=False)
    (run/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    if args.result_file is not None:
        with args.result_file.open('x') as result:
            json.dump({'run':str(run),'hash_rule':hash_rule},result)
    print('Committed masked refund delivery verified:',run,flush=True)


if __name__=='__main__': main()
