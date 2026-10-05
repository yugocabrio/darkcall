#!/usr/bin/env python3
"""Actual local-regtest funding -> native PCZT/FROST -> mined recipient note.

Own test coins only. Public endpoints and synthetic default native-gate outputs
are never accepted by this driver. This is a threshold transfer, not an auction.
"""
import argparse
import json
import pathlib
import subprocess
from regtest_node import ROOT, GENESIS, rpc, status
from regtest_funding import tree
from regtest_deposits import verify_note

NATIVE = ROOT / 'target/release/darkcall-native-gate'
WALLET = ROOT / 'target/release/examples/regtest_wallet'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_wallet', type=pathlib.Path)
    parser.add_argument('private_notes', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected a simple new local run name')
    s = status()
    notes = json.loads(args.private_notes.read_text())
    if notes['genesis'] != GENESIS or notes['tip_hash'] != s['tip'] or notes['tip_height'] != s['blocks']:
        raise RuntimeError('Source witnesses are not the current pinned local-regtest snapshot')
    state = rpc('z_gettreestate', [s['tip']])
    if bytes(notes['anchor']).hex() != state['ironwood']['commitments']['finalRoot']:
        raise RuntimeError('Source anchor differs from actual full-node root')
    bank = json.loads(subprocess.check_output([str(WALLET), 'init', str(args.source_wallet)], cwd=ROOT, text=True))
    note = next(n for n in notes['notes']
        if n['recipient'] in [bank['receiver'], bank['internal_receiver']]
        and (not n.get('coinbase', True) or s['blocks'] >= n['mined_height'] + 100))
    verify_note(note)
    recipient = ROOT / '.local' / (args.name + '-recipient')
    subprocess.check_output([str(WALLET), 'init', str(recipient)], cwd=ROOT, text=True)
    checkpoint = tree(state)
    # Fail a changed witness BEFORE allowing the real transaction to be proved.
    bad = dict(notes)
    bad['anchor'] = list(notes['anchor'])
    bad['anchor'][0] ^= 1
    bad_path = args.source_wallet / (args.name + '-bad-anchor.json')
    with bad_path.open('x') as f:
        json.dump(bad, f)
    bad_path.chmod(0o600)
    negative = subprocess.run([str(NATIVE), '--regtest-spend', str(args.source_wallet), str(bad_path),
                               str(recipient), args.name + '-reject'], cwd=ROOT, capture_output=True, text=True,
                              env={**__import__('os').environ, 'RAYON_NUM_THREADS': '2'}, timeout=60)
    if negative.returncode == 0 or 'funding witness differs' not in negative.stderr:
        raise RuntimeError('Changed native witness was not rejected before proving')
    bad_path.unlink()
    construction = json.loads(subprocess.check_output([str(NATIVE), '--regtest-spend', str(args.source_wallet),
        str(args.private_notes), str(recipient), args.name], cwd=ROOT, text=True,
        env={**__import__('os').environ, 'RAYON_NUM_THREADS': '2'}, timeout=600))
    if status()['tip'] != s['tip']:
        raise RuntimeError('Local chain changed during construction; reconcile before broadcast')
    directory = ROOT / '.local' / args.name
    provenance = json.loads((directory / 'private-provenance.json').read_text())
    if provenance['source_cmx'] != note['cmx']:
        raise RuntimeError('Native builder selected another source note')
    binary = (directory / 'regtest-transaction.bin').read_bytes()
    # This call is hardwired to loopback and preceded by genesis/upgrade/source
    # guards. Do not use this script for mainnet or external testnet custody.
    accepted = rpc('sendrawtransaction', [binary.hex()], timeout=180)
    if accepted != construction['txid']:
        raise RuntimeError('Full node returned another transaction identity')
    rpc('generate', [2], timeout=300)
    mined = rpc('getrawtransaction', [accepted, 1])
    if mined['confirmations'] < 2 or not mined.get('in_active_chain', False):
        raise RuntimeError('Native transaction not confirmed in canonical local chain')
    final_status = status()
    blocks = []
    for height in range(checkpoint['height'] + 1, final_status['blocks'] + 1):
        h = rpc('getblockhash', [height])
        b = rpc('getblock', [h, 2])
        blocks.append({'height': height, 'hash': h, 'prev_hash': b['previousblockhash'], 'time': b['time'],
                       'ironwood_size': b['trees']['ironwood']['size'], 'transactions': [tx['hex'] for tx in b['tx']]})
    capture = {'genesis': GENESIS, 'branch': final_status['consensus']['chaintip'], 'nu6_3_height': 2,
               'initial': checkpoint, 'final_tree': tree(rpc('z_gettreestate', [final_status['tip']])), 'blocks': blocks}
    capture_path = recipient / f'node-capture-{final_status["blocks"]}.json'
    with capture_path.open('x') as f:
        json.dump(capture, f)
    capture_path.chmod(0o600)
    scan = json.loads(subprocess.check_output([str(WALLET), 'scan', str(recipient), str(capture_path)], cwd=ROOT, text=True))
    if scan['balance_zatoshi'] != construction['recipient_amount'] or scan['decrypted_unspent_native_notes'] != 1:
        raise RuntimeError('Recipient did not receive the exact native transfer')
    summary = {'scope': 'actual isolated Zebra regtest threshold spend; not standard testnet or auction settlement',
               'source_funding_canonical_and_mature': True, 'changed_witness_rejected_before_proving': True,
               'native_construction': construction, 'broadcast_accepted_by_full_node': True,
               'canonical_confirmations': mined['confirmations'], 'recipient_scan': scan,
               'private_keys_or_openings_committed': False,
               'finality_scope': 'two controlled-regtest confirmations; no public-chain probabilistic finality claim'}
    (directory / 'confirmed-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
