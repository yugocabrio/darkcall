#!/usr/bin/env python3
"""How DarkCall's REP3 circuits grow with the number of bids (measurement only).

The committed circuits are fixed at four bids. This derives N-bid variants of
the three coNoir circuits (clearing, winner-flag delivery and the Poseidon2
settlement plan with N + 2 rows) under ignored .local/, compiles them with the
pinned Noir and counts UltraHonk gates with the pinned bb. A variant changes
only the bid count; the four-bid variants must match the committed circuits
in ACIR opcodes, gates and padded size, or the run stops. With --mpc it also times the clearing
circuit's three-party REP3 witness and proof on synthetic bids, one size at a
time, and verifies the proof. Synthetic fixture values only, never real bids.
"""
import argparse
import json
import os
import pathlib
import re
import signal
import socket
import subprocess
import time
from mpc_process import TIME, peak_rss, separate_session, signal_child

ROOT = pathlib.Path(__file__).resolve().parents[1]
NARGO = ROOT / '.deps/noir-1.0.0-rc.2/nargo'
BB = ROOT / '.deps/barretenberg-src/barretenberg/cpp/build-darkcall/bin/bb'
UPSTREAM = ROOT / '.deps/co-snarks'
CONOIR = UPSTREAM / 'target-aes-native/release/co-noir'
CRS = UPSTREAM / 'co-noir/co-noir-common/src/crs'
PIN = '467a2abbad940d40ed78f38d424cc21920988976'
COMMITTED = {'clearing': 'auction4', 'flags': 'delivery_flags4', 'plan': 'settlement_poseidon4'}

ACCEPTED_ROOT = '''    let mut input = [0; ACCEPTED_INPUT];
    input[0] = D_ACCEPTED;
    input[1] = ctx;
    input[2] = N as Field;
    input[3] = n as Field;
    for i in 0..N {
        input[4 + i] = active[i] as Field;
        input[4 + N + i] = commitments[i];
    }
    hash(input)'''

CLEARING = '''use darkcall::{auction, context, verify_accepted};
global N: u32 = @N@;

fn main(
    bids: [u64; N],
    salts: [Field; N],
    condition_digest: pub [u8; 32],
    ids: pub [[u8; 32]; N],
    active: pub [bool; N],
    units: pub u32,
    reserve: pub u64,
    budget: pub u64,
    processing_fee: pub u64,
    commitments: pub [Field; N],
    root: pub Field,
) -> pub (u64, u32) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    verify_accepted(bids, salts, ids, active, ctx, commitments, root);
    let result = auction(bids, ids, active, units, reserve, budget, processing_fee);
    (result.price, result.sold)
}
'''

FLAGS = '''use darkcall::{auction, context, verify_accepted, limbs, hash, id_less};
global N: u32 = @N@;
global D_WINNER: Field = 0x4461726b43616c6c2f77696e6e65722d6d61736b2f7632;
global D_MASK: Field = 0x4461726b43616c6c2f66696c6c2d6d61736b2f7632;
pub fn mask_commitment(ctx: Field, root: Field, id: [u8; 32], mask: Field, blind: Field) -> Field {
    let ticket = limbs(id);
    hash([D_MASK, ctx, root, ticket[0], ticket[1], ticket[2], ticket[3], mask, blind])
}
pub fn winner_mask_commitment(ctx: Field, root: Field, id: [u8; 32], mask: Field, blind: Field) -> Field {
    let ticket = limbs(id);
    hash([D_WINNER, ctx, root, ticket[0], ticket[1], ticket[2], ticket[3], mask, blind])
}
fn winning_flags(bids: [u64; N], ids: [[u8; 32]; N], units: u32, reserve: u64) -> [bool; N] {
    let mut flags = [false; N];
    for i in 0..N {
        let mut rank = 0;
        for j in 0..N {
            let ahead = (bids[j] > bids[i]) | ((bids[j] == bids[i]) & id_less(ids[j], ids[i]));
            if (bids[j] >= reserve) & ahead { rank += 1; }
        }
        flags[i] = (bids[i] >= reserve) & (rank < units);
    }
    flags
}
fn main(
    bids: [u64; N], salts: [Field; N],
    output_masks: [Field; N], output_blinds: [Field; N], winner_masks: [Field; N], winner_blinds: [Field; N],
    condition_digest: pub [u8; 32], ids: pub [[u8; 32]; N], units: pub u32,
    reserve: pub u64, budget: pub u64, processing_fee: pub u64, commitments: pub [Field; N], root: pub Field,
    mask_commitments: pub [Field; N], winner_mask_commitments: pub [Field; N],
) -> pub (u64, u32, [Field; N], [Field; N]) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    verify_accepted(bids, salts, ids, [true; N], ctx, commitments, root);
    let result = auction(bids, ids, [true; N], units, reserve, budget, processing_fee);
    let mut masked = [0; N];
    let flags = winning_flags(bids, ids, units, reserve);
    let mut masked_flags = [0; N];
    let mut won_count = 0;
    for i in 0..N {
        assert(mask_commitment(ctx, root, ids[i], output_masks[i], output_blinds[i]) == mask_commitments[i]);
        masked[i] = result.refunds[i] as Field + output_masks[i];
        assert(winner_mask_commitment(ctx, root, ids[i], winner_masks[i], winner_blinds[i]) == winner_mask_commitments[i]);
        masked_flags[i] = flags[i] as Field + winner_masks[i];
        won_count += flags[i] as u32;
        assert(result.refunds[i] == budget - if flags[i] { result.price } else { 0 });
    }
    assert(won_count == result.sold);
    (result.price, result.sold, masked, masked_flags)
}
'''

PLAN = '''use darkcall::{auction, context, verify_accepted, limbs, hash};
use rows::{Row, encode};
global N: u32 = @N@;
global R: u32 = @N@ + 2;
global PLAN_INPUT: u32 = 8 + 8 * R;

pub fn commitment(row: Row) -> [u32; 8] {
    let input = encode(row);
    let mut packed = [0; 8];
    for i in 0..8 { for j in 0..30 {
        let index = i * 30 + j;
        if index < 224 { packed[i] = packed[i] * 256 + input[index] as Field; }
    } }
    let bytes: [u8; 32] = hash(packed).to_be_bytes();
    let mut words = [0; 8];
    for i in 0..8 { for j in 0..4 { words[i] = words[i] * 256 + bytes[i*4+j] as u32; } }
    words
}

global D_RECEIVER: Field = 0x4461726b43616c6c2f72656365697665722f7631;
global D_PLAN: Field = 0x4461726b43616c6c2f706c616e2f7631;

pub fn receiver_commitment(ctx: Field, ticket: [u8; 32], role: u32, gd: [u8; 32], pkd: [u8; 32], nonce: Field) -> Field {
    let id = limbs(ticket); let g = limbs(gd); let p = limbs(pkd);
    hash([D_RECEIVER, ctx, id[0], id[1], id[2], id[3], role as Field, g[0], g[1], g[2], g[3], p[0], p[1], p[2], p[3], nonce])
}

fn main(
    bids: [u64; N], salts: [Field; N],
    gd: [[u8; 32]; R], pkd: [[u8; 32]; R], receiver_nonces: [Field; R], row_salts: [[u8; 32]; R],
    condition_digest: pub [u8; 32], ids: pub [[u8; 32]; N], units: pub u32,
    reserve: pub u64, budget: pub u64, processing_fee: pub u64,
    commitments: pub [Field; N], root: pub Field,
    receivers: pub [Field; R], action_indices: pub [u32; R],
) -> pub (u64, u32, Field, [[u32; 8]; R]) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    verify_accepted(bids, salts, ids, [true; N], ctx, commitments, root);
    let result = auction(bids, ids, [true; N], units, reserve, budget, processing_fee);
    let mut amounts: [u64; R] = [0; R];
    for i in 0..N { amounts[i] = result.refunds[i]; }
    amounts[N] = result.seller_amount;
    amounts[N + 1] = result.operator_amount;
    let root_bytes: [u8; 32] = root.to_be_bytes();
    let mut digests = [[0; 8]; R];
    for i in 0..R {
        assert(action_indices[i] < R);
        for j in 0..i { assert(action_indices[j] != action_indices[i]); }
        let role = if i < N { 0 } else { i - (N - 1) };
        let ticket = if i < N { ids[i] } else { [0; 32] };
        assert(receiver_commitment(ctx, ticket, role, gd[i], pkd[i], receiver_nonces[i]) == receivers[i]);
        digests[i] = commitment(Row {
            conditions: condition_digest, accepted_root: root_bytes, ticket,
            role, action_index: action_indices[i], gd: gd[i], pkd: pkd[i],
            amount: amounts[i], salt: row_salts[i],
        });
    }
    let mut plan_fields = [0; PLAN_INPUT];
    plan_fields[0] = D_PLAN; plan_fields[1] = ctx; plan_fields[2] = root;
    plan_fields[3] = result.price as Field; plan_fields[4] = result.sold as Field;
    plan_fields[5] = result.action_count as Field; plan_fields[6] = result.network_fee as Field;
    plan_fields[7] = result.operator_amount as Field;
    for i in 0..R { for j in 0..8 { plan_fields[8 + i * 8 + j] = digests[i][j] as Field; } }
    (result.price, result.sold, hash(plan_fields), digests)
}
'''

FIXTURE = '''use darkcall::{context, bid_commitment, accepted_root};
global N: u32 = @N@;

fn main(bids: [u64; N], salts: [Field; N], condition_digest: [u8; 32], ids: [[u8; 32]; N],
        units: u32, reserve: u64, budget: u64, processing_fee: u64) -> pub ([Field; N], Field) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    let mut commitments = [0; N];
    for i in 0..N { commitments[i] = bid_commitment(ctx, ids[i], bids[i], salts[i]); }
    (commitments, accepted_root(ctx, [true; N], commitments))
}
'''


def library(n):
    source = (ROOT / 'circuits/protocol_lib/src/lib.nr').read_text()
    source = source[:source.index('#[test]')]
    if source.count('global N: u32 = 4;') != 1:
        raise RuntimeError('Committed library no longer fixes N = 4 in one place')
    source = source.replace('global N: u32 = 4;', f'global N: u32 = {n};\nglobal ACCEPTED_INPUT: u32 = 4 + 2 * N;')
    line = re.search(r'\n    hash\(\[D_ACCEPTED, ctx, N as Field, n as Field,[^\n]*\]\)\n', source)
    if not line:
        raise RuntimeError('Committed accepted_root changed; update the generalisation')
    return source.replace(line.group(0), '\n' + ACCEPTED_ROOT + '\n')


def package(path, name, main, deps):
    (path / 'src').mkdir(parents=True)
    lines = ['[package]', f'name = "{name}"', 'type = "bin"', 'authors = []', '[dependencies]']
    lines += [f'{dep} = {{ path = "{target}" }}' for dep, target in deps.items()]
    (path / 'Nargo.toml').write_text('\n'.join(lines) + '\n')
    (path / 'src/main.nr').write_text(main)


def variant(base, n):
    folder = base / f'n{n}'
    lib = folder / 'protocol_lib'
    (lib / 'src').mkdir(parents=True)
    (lib / 'Nargo.toml').write_text('[package]\nname = "darkcall"\ntype = "lib"\nauthors = []\n[dependencies]\n')
    (lib / 'src/lib.nr').write_text(library(n))
    rows = folder / 'settlement_lib'
    (rows / 'src').mkdir(parents=True)
    (rows / 'Nargo.toml').write_text('[package]\nname = "darkcall_settlement"\ntype = "lib"\nauthors = []\n[dependencies]\n')
    text = (ROOT / 'circuits/settlement_lib/src/lib.nr').read_text()
    (rows / 'src/lib.nr').write_text(text[:text.index('#[test]')] if '#[test]' in text else text)
    for key, template in (('clearing', CLEARING), ('flags', FLAGS), ('plan', PLAN), ('fixture', FIXTURE)):
        deps = {'darkcall': '../protocol_lib'}
        if key == 'plan':
            deps['rows'] = '../settlement_lib'
        package(folder / key, f'scaling_{key}{n}', template.replace('@N@', str(n)), deps)
    return folder


def compile_program(path):
    subprocess.run([str(NARGO), 'compile', '--program-dir', str(path)], check=True, capture_output=True, timeout=600)
    return next((path / 'target').glob('*.json'))


def gates(program):
    out = subprocess.check_output([str(BB), 'gates', '--scheme', 'ultra_honk', '-b', str(program)], text=True, timeout=600)
    info = json.loads(out)['functions'][0]
    dyadic = subprocess.check_output([str(CONOIR), 'circuit-info', '--circuit', str(program)], text=True, timeout=600)
    return {'acir_opcodes': info['acir_opcodes'], 'gates': info['circuit_size'],
            'dyadic_size': int(re.search(r'circuit_size = (\d+)', dyadic).group(1))}


def toml_value(value):
    if isinstance(value, list):
        return '[' + ', '.join(toml_value(v) for v in value) + ']'
    if isinstance(value, bool):
        return 'true' if value else 'false'
    return json.dumps(str(value))


def write_toml(path, values):
    path.write_text(''.join(f'{key} = {toml_value(value)}\n' for key, value in values.items()))


def synthetic(n):
    # Descending distinct bids, all above the reserve; one more bid than units.
    return {'bids': [95 - 5 * i for i in range(n)], 'salts': [1000 + i for i in range(n)],
            'condition_digest': list(range(1, 33)), 'ids': [[0] * 31 + [i + 1] for i in range(n)],
            'units': n - 1, 'reserve': 20, 'budget': 100, 'processing_fee': 15000}


def execute(program_dir, values):
    write_toml(program_dir / 'Prover.toml', values)
    out = subprocess.run([str(NARGO), 'execute', '--program-dir', str(program_dir)],
                         check=True, capture_output=True, text=True, timeout=600).stdout
    return re.findall(r'0x[0-9a-f]+|\b\d+\b', out.split('Circuit output:', 1)[1])


def mpc_clearing(folder, program, n, threads, summary_path, summary):
    values = synthetic(n)
    outputs = execute(folder / 'fixture', values)
    commitments, root = outputs[:n], outputs[n]
    inputs = {**values, 'active': [True] * n, 'commitments': commitments, 'root': root}
    plain = execute(folder / 'clearing', inputs)
    expected = (int(plain[0], 0), int(plain[1], 0))
    if expected != (values['bids'][-1], n - 1):
        raise RuntimeError('Synthetic clearing did not give the expected price and units')
    run = folder / 'mpc'
    run.mkdir()
    write_toml(run / 'Prover.toml', inputs)
    environment = {**os.environ, 'RUST_LOG': 'warn', 'RAYON_NUM_THREADS': str(threads)}
    stages = []

    def record(name, commands, timeout):
        start = time.monotonic()
        children, handles = [], []
        try:
            for i, command in enumerate(commands):
                handle = (run / f'{name}-{i}.log').open('wb')
                handles.append(handle)
                children.append(subprocess.Popen([*TIME, str(CONOIR), *map(str, command)],
                    stdout=handle, stderr=subprocess.STDOUT, env=environment, cwd=run, start_new_session=separate_session()))
            expires = time.monotonic() + timeout
            while any(child.poll() is None for child in children):
                if any(child.poll() not in (None, 0) for child in children):
                    raise RuntimeError(name + ' failed')
                if time.monotonic() >= expires:
                    raise TimeoutError(name + ' exceeded its bound')
                time.sleep(0.2)
            if any(child.wait() for child in children):
                raise RuntimeError(name + ' failed')
        finally:
            for child in children:
                if child.poll() is None:
                    signal_child(child)
            for child in children:
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    signal_child(child, signal.SIGKILL)
                    child.wait()
            for handle in handles:
                handle.close()
        peaks = [peak_rss((run / f'{name}-{i}.log').read_text(errors='replace')) for i in range(len(commands))]
        stages.append({'name': name, 'seconds': round(time.monotonic() - start, 3),
                       'peak_rss_mib': [round(p / 2 ** 20) for p in peaks]})
        summary['mpc'][str(n)] = {'stages': stages, 'price_and_units': list(expected), 'rayon_threads': threads}
        summary_path.write_text(json.dumps(summary, indent=2) + '\n')

    record('split', [['split-input', '--circuit', program, '--input', run / 'Prover.toml', '--protocol', 'REP3', '--out-dir', run]], 600)
    held, ports = [], []
    for _ in range(3):
        sock = socket.socket()
        sock.bind(('127.0.0.1', 0))
        held.append(sock)
        ports.append(sock.getsockname()[1])
    for sock in held:
        sock.close()
    data = UPSTREAM / 'co-noir/co-noir/examples/data'
    for party in range(3):
        text = f'[network]\nmy_id = {party}\nbind_addr = "127.0.0.1:{ports[party]}"\nmax_frame_length = 469762056\n'
        for peer, port in enumerate(ports):
            text += f'\n[[network.parties]]\nid = {peer}\ndns_name = "127.0.0.1:{port}"\n'
        text += '\n[network.tls]\nkey = ' + json.dumps(str(data / f'key{party}.der'))
        text += '\ncerts = ' + json.dumps([str(data / f'cert{i}.der') for i in range(3)]) + '\n'
        (run / f'party{party}.toml').write_text(text)
    record('witness', [['generate-witness', '--circuit', program, '--protocol', 'REP3', '--input', run / f'Prover.toml.{i}.shared',
                        '--config', run / f'party{i}.toml', '--out', run / f'witness.{i}.shared'] for i in range(3)], 3600)
    record('vk', [['create-vk', '--circuit', program, '--crs', CRS / 'bn254_g1.dat', '--hasher', 'poseidon2', '--vk', run / 'vk']], 1800)
    record('proof', [['build-and-generate-proof', '--circuit', program, '--protocol', 'REP3', '--witness', run / f'witness.{i}.shared',
                      '--config', run / f'party{i}.toml', '--crs', CRS / 'bn254_g1.dat', '--hasher', 'poseidon2',
                      '--vk', run / 'vk', '--zk', '--out', run / f'proof.{i}', '--public-input', run / f'public-input{i}'] for i in range(3)], 3600)
    record('verify', [['verify', '--proof', run / 'proof.0', '--public-input', run / 'public-input0', '--vk', run / 'vk',
                       '--crs', CRS / 'bn254_g2.dat', '--hasher', 'poseidon2', '--has-zk']], 600)
    summary['mpc'][str(n)]['verified'] = True
    summary_path.write_text(json.dumps(summary, indent=2) + '\n')


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sizes', type=int, nargs='+', default=[4, 8, 16])
    parser.add_argument('--mpc', type=int, nargs='*', default=[], help='bid counts whose clearing proof is run in REP3, in order')
    parser.add_argument('--threads', type=int, default=2)
    args = parser.parse_args()
    if 4 not in args.sizes or any(n < 2 or n > 32 for n in args.sizes) or not set(args.mpc) <= set(args.sizes):
        raise RuntimeError('Sizes must include 4, stay within 2..32 and cover every MPC size')
    if subprocess.check_output(['git', '-C', str(UPSTREAM), 'rev-parse', 'HEAD'], text=True).strip() != PIN:
        raise RuntimeError('Build the pinned coNoir first')
    if 'nargo version = 1.0.0-rc.2\n' not in subprocess.check_output([str(NARGO), '--version'], text=True):
        raise RuntimeError('Wrong Noir version')
    base = ROOT / '.local' / ('bid-scaling-' + time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()))
    base.mkdir(mode=0o700)
    summary_path = base / 'summary.json'
    summary = {'scope': 'gate counts of N-bid variants of the committed REP3 circuits; synthetic MPC timing',
               'source_pin': PIN, 'noir': '1.0.0-rc.2', 'committed': {}, 'variants': {}, 'mpc': {}}
    for key, name in COMMITTED.items():
        summary['committed'][key] = gates(compile_program(ROOT / 'circuits' / name))
    programs = {}
    for n in sorted(args.sizes):
        folder = variant(base, n)
        programs[n] = (folder, {key: compile_program(folder / key) for key in COMMITTED})
        summary['variants'][str(n)] = {key: gates(path) for key, path in programs[n][1].items()}
        summary_path.write_text(json.dumps(summary, indent=2) + '\n')
    for key in COMMITTED:
        if summary['variants']['4'][key] != summary['committed'][key]:
            raise RuntimeError(f'Four-bid {key} variant differs from the committed circuit in opcodes, gates or padded size')
    summary['four_bid_variants_match_committed'] = True
    summary_path.write_text(json.dumps(summary, indent=2) + '\n')
    for n in args.mpc:
        folder, compiled = programs[n]
        mpc_clearing(folder, compiled['clearing'], n, args.threads, summary_path, summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
