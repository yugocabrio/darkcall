#!/usr/bin/env python3
"""Fresh actual NU7 coinbase notes and canonical paths on a separate LOCAL node.

No public network funds. Uses an ordinary local bank key, not the DKG escrow.
Captures are trusted-node data; the SDK scanner checks native encodings/tree links.
"""
import argparse
import json
import os
import pathlib
import subprocess
from regtest_nu7_node import ROOT, GENESIS, BRANCH, rpc, status
from regtest_funding import tree
from regtest_deposits import private_read, private_write, ENV

WALLET = ROOT/'target/release/examples/regtest_wallet'
SDK = ROOT/'.deps/nu7-compat-target/release/darkcall-nu7-compat'


def capture(wallet, initial, snapshot):
    blocks = []
    if initial['height'] < 2 or snapshot['blocks']-initial['height'] > 500:
        raise RuntimeError('NU7 capture requires activated, bounded canonical history')
    if rpc('getblockhash', [initial['height']]) != initial['hash']:
        raise RuntimeError('NU7 scanning birthday is no longer canonical')
    for height in range(initial['height']+1, snapshot['blocks']+1):
        h = rpc('getblockhash', [height])
        block = rpc('getblock', [h, 2])
        blocks.append({'height':height, 'hash':h, 'prev_hash':block['previousblockhash'],
            'time':block['time'], 'ironwood_size':block['trees']['ironwood']['size'],
            'transactions':[tx['hex'] for tx in block['tx']]})
    final = tree(rpc('z_gettreestate', [snapshot['tip']]))
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('NU7 chain changed while capturing')
    data = {'genesis':GENESIS, 'branch':BRANCH, 'nu7_height':2,
        'initial':initial, 'final_tree':final, 'blocks':blocks}
    path = wallet/f'nu7-node-capture-{snapshot["blocks"]}.json'
    if path.exists():
        if json.loads(private_read(path, 100_000_000)) != data:
            raise RuntimeError('Existing NU7 capture differs; preserve it')
    else:
        private_write(path, data)
    result = json.loads(subprocess.check_output(
        [str(SDK), 'regtest-scan', str(wallet), str(path)], cwd=ROOT, env=ENV,
        text=True, timeout=600))
    return result, wallet/f'private-nu7-notes-{snapshot["blocks"]}.json'


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh simple ignored NU7 bank wallet name required')
    wallet = ROOT/'.local'/args.name
    if wallet.exists():
        raise RuntimeError('Wallet exists; preserve its birthday, keys and captures')
    before = status()
    if before['blocks'] < 2:
        raise RuntimeError('Activate separate local NU7 before funding')
    initialized = json.loads(subprocess.check_output(
        [str(WALLET), 'init', str(wallet)], cwd=ROOT, text=True, env=ENV))
    initial = tree(rpc('z_gettreestate', [before['tip']]))
    private_write(wallet/'nu7-birthday.json', initial)
    if status()['tip'] != before['tip']:
        raise RuntimeError('NU7 chain changed before funding')
    rpc('generatetoaddress', [2, initialized['address']], timeout=300)
    status()  # Recheck the distinct genesis/branch before maturity mining.
    rpc('generate', [100], timeout=300)
    snapshot = status()
    scan, openings = capture(wallet, initial, snapshot)
    notes = json.loads(private_read(openings))
    if (scan['decrypted_unspent_native_notes'] != 2
            or any(not n['coinbase'] or snapshot['blocks'] < n['mined_height']+100
                   for n in notes['notes'])):
        raise RuntimeError('Two actual matured native coinbase notes not received')
    result = {'scope':'actual matured native NU7 funding; separate isolated regtest',
        'node':snapshot, 'scan':scan, 'two_coinbase_notes_mature':True,
        'public_testnet':False, 'dkg_escrow':False, 'auction_settlement':False,
        'ordinary_bank_key_only':True, 'old_regtest_changed':False}
    private_write(wallet/'nu7-funding-summary.json', result)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
