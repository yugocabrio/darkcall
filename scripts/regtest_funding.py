#!/usr/bin/env python3
"""Mine actual native funding notes and scan canonical witnesses on LOCAL regtest.

Private keys/openings stay ignored. Never sends a transaction to a public chain.
"""
import argparse
import json
import pathlib
import subprocess
from regtest_node import ROOT, rpc, status

BIN = ROOT / 'target/release/examples/regtest_wallet'


def tree(state):
    return {'height': state['height'], 'hash': state['hash'],
            'ironwood_tree': state['ironwood']['commitments']['finalState']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected a simple local wallet name')
    wallet = ROOT / '.local' / args.name
    s = status()
    if s['blocks'] < 2:
        raise RuntimeError('Activate local NU6.3 before funding')
    init = json.loads(subprocess.check_output([str(BIN), 'init', str(wallet)], cwd=ROOT, text=True))
    birthday = wallet / 'birthday.json'
    if birthday.exists():
        initial = json.loads(birthday.read_text())
    else:
        initial = tree(rpc('z_gettreestate', [s['tip']]))
        birthday.write_text(json.dumps(initial) + '\n')
        birthday.chmod(0o600)
        # Mine directly to a shielded UA: Zebra routes its Orchard-shaped receiver
        # to Ironwood in the active native rules. The default miner remains a
        # disposable transparent test receiver for the subsequent maturity blocks.
        rpc('generatetoaddress', [2, init['address']], timeout=300)
        rpc('generate', [100], timeout=300)
    s = status()
    blocks = []
    for height in range(initial['height'] + 1, s['blocks'] + 1):
        if len(blocks) >= 500:
            raise RuntimeError('Bounded funding gate requires a new capture strategy')
        h = rpc('getblockhash', [height])
        block = rpc('getblock', [h, 2])
        blocks.append({'height': height, 'hash': h, 'prev_hash': block['previousblockhash'],
                       'time': block['time'], 'ironwood_size': block['trees']['ironwood']['size'],
                       'transactions': [tx['hex'] for tx in block['tx']]})
    final = tree(rpc('z_gettreestate', [s['tip']]))
    # Fail if the chain changed while capturing. A caller must reconcile/retry;
    # do not silently scan mixed branches.
    if status()['tip'] != s['tip']:
        raise RuntimeError('regtest tip changed during capture')
    capture = {'genesis': s['genesis'], 'branch': s['consensus']['chaintip'], 'nu6_3_height': 2,
               'initial': initial, 'final_tree': final, 'blocks': blocks}
    path = wallet / f'node-capture-{s["blocks"]}.json'
    if path.exists():
        raise RuntimeError('Capture exists; preserve it and choose an explicit replay')
    path.write_text(json.dumps(capture) + '\n')
    path.chmod(0o600)
    result = json.loads(subprocess.check_output([str(BIN), 'scan', str(wallet), str(path)], cwd=ROOT, text=True))
    (wallet / 'scan-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
