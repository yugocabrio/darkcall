#!/usr/bin/env python3
"""Six-row SHA/Poseidon comparison: committed REP3 bids -> plan proof -> native row proof.

Synthetic, output-only native bundle. This is not funded Zcash settlement.
Private fixture data stays under .local; no bidder plaintext is read here.
"""
import argparse
import hashlib
import json
import os
import pathlib
import signal
import socket
import subprocess
import time
from mpc_process import TIME, peak_rss, separate_session, signal_child

ROOT = pathlib.Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / '.deps/co-snarks'
PIN = '467a2abbad940d40ed78f38d424cc21920988976'
BIN = UPSTREAM / 'target-aes-native/release/co-noir'
NARGO = ROOT / '.deps/noir-1.0.0-rc.2/nargo'
INPUT = ROOT / 'target/release/darkcall-input-tool'
CLIENT = ROOT / 'circuits/bid_input/target/darkcall_bid_input.json'
CRS = UPSTREAM / 'co-noir/co-noir-common/src/crs'


def main():
    os.umask(0o077)  # Includes coNoir-created secret shares/witnesses.
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--book-run', type=pathlib.Path, required=True)
    parser.add_argument('--native-run', type=pathlib.Path, required=True)
    parser.add_argument('--hash', choices=['sha', 'poseidon'], default='sha')
    parser.add_argument('--result-file', type=pathlib.Path)
    parser.add_argument('--receiver-openings', type=pathlib.Path)
    args = parser.parse_args()
    circuit_name = 'settlement4' if args.hash == 'sha' else 'settlement_poseidon4'
    circuit_path = ROOT / f'circuits/{circuit_name}/target/darkcall_{circuit_name}.json'
    native_binary = ROOT / f'target/release/{args.hash}_batch'
    hash_rule = 'sha256-row-v1' if args.hash == 'sha' else 'bn254-poseidon2-packed-row-v1'
    book, native = args.book_run.resolve(), args.native_run.resolve()
    if args.result_file is not None and not args.result_file.resolve().is_relative_to(ROOT / '.local'):
        raise RuntimeError('Result file must remain in ignored local fixtures')
    if any(not p.is_relative_to(ROOT / '.local') for p in [book, native]):
        raise RuntimeError('Only ignored synthetic local runs are supported')
    if subprocess.check_output(['git', '-C', str(UPSTREAM), 'rev-parse', 'HEAD'], text=True).strip() != PIN:
        raise RuntimeError('Wrong coNoir source pin')
    if 'nargo version = 1.0.0-rc.2\n' not in subprocess.check_output([str(NARGO), '--version'], text=True):
        raise RuntimeError('Wrong Noir version')
    for circuit in [circuit_name, 'bid_input']:
        subprocess.run([str(NARGO), 'compile', '--program-dir', str(ROOT / 'circuits' / circuit)], check=True)
    native_summary = json.loads((native / 'native-proof-summary.json').read_text())
    if not native_summary.get('proof_verified') or not native_summary.get('book_context_supplied'):
        raise RuntimeError('Finish the book-context native proof first')
    if native_summary.get('hash_rule', 'sha256-row-v1') != hash_rule:
        raise RuntimeError('Native proof uses a different hash rule')
    run = ROOT / '.local' / ('mpc-' + circuit_name + '-' + time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()))
    run.mkdir(mode=0o700)
    summary = dict(scope='six-row comparison with output-only native bundle', hash_rule=hash_rule, circuit=circuit_name, source_pin=PIN,
                   native_binding_layout=native_summary.get('layout','working-33-column-v1'),
                   independent_hosts=False, funded_transaction=False, custody_signatures=False,
                   input_proofs_verified=False, native_row_proof_verified=False,
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
                children.append(subprocess.Popen([*TIME, str(binary), *map(str,command)],
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
                stage['peak_rss_bytes_per_process'].append(peak_rss(log.read_text(errors='replace')))
            summary['stages'].append(stage)
            (run/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')

    record('native-proof-verification', [['--verify-export',native]], binary=native_binary)
    summary['native_row_proof_verified'] = True
    plan_command = ['plan-fixture', book/'book.json', native/'native-private-rows.json', run/'plan']
    if args.receiver_openings is not None:
        if not args.receiver_openings.resolve().is_relative_to(ROOT / '.local'):
            raise RuntimeError('Receiver openings must stay ignored')
        plan_command.append(args.receiver_openings.resolve())
    record('plan-fixture', [plan_command], binary=INPUT)
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
    if len(fields)!=232: raise RuntimeError('Unexpected statement size')
    if native_summary.get('private_fill_delivery_verified') and fields[181:183]!=[native_summary['price'],native_summary['sold']]: raise RuntimeError('Clearing price/sold changed after private delivery')
    record('native-plan-link',[['link-plan',book/'book.json',native/'native-public.json',run/'public-input0']],binary=INPUT)
    for name,index in [('price',181),('plan',183),('row',184)]:
        changed=bytearray(encoded); changed[index*32:(index+1)*32]=(fields[index]^1).to_bytes(32,'big')
        (run/f'tampered-{name}').write_bytes(changed)
        log=run/f'reject-{name}.log'
        with log.open('wb') as out:
            result=subprocess.run([str(BIN),*map(str,[*verify,'--public-input',run/f'tampered-{name}'])],stdout=out,stderr=subprocess.STDOUT,env=env,timeout=300)
        if result.returncode==0 or 'Proof verification failed' not in log.read_text(): raise RuntimeError('No cryptographic rejection of '+name)
    summary.update(passed=True,price=fields[181],sold=fields[182],rows=6,native_row_hashes_match=True,
        proof_bytes=(run/'proof.0').stat().st_size,changed_price_plan_row_rejected=True,
        external_barretenberg_verified=False,funding_authenticated=False,receiver_receipts_authenticated=False)
    (run/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    if args.result_file is not None:
        with args.result_file.open('x') as result:
            json.dump({'run':str(run),'hash_rule':hash_rule},result)
    print('Complete synthetic '+args.hash+'/native row bridge verified:',run,flush=True)


if __name__=='__main__': main()
