#!/usr/bin/env python3
"""One certified funded NU7 memo auction settled on PUBLIC TESTNET.

The testnet counterpart of regtest_nu7_settlement.py: the same native gate
(fresh REP3 delivery, six output bindings, DKG FROST) under the same resource
guard. On a live chain the tip keeps moving, so the driver re-checks that the
spent notes' snapshot and anchor are still canonical before broadcast, then
waits for two real confirmations instead of mining them. Never substitutes a
transaction after authorization; journals record broadcast and finalization.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_resources import guarded_json_call
from testnet_chain import SDK, capture, send, source_check, tree_at_tip, wait_confirmations

NATIVE = ROOT / 'target/release/darkcall-native-gate'


def call(*args):
    return json.loads(subprocess.check_output([str(SDK), *map(str, args)],
        cwd=ROOT, env=ENV, text=True, timeout=600))


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('escrow', 'notes', 'book', 'receivers', 'admission'):
        parser.add_argument(key, type=pathlib.Path)
    parser.add_argument('--recipient-wallet', type=pathlib.Path, action='append', required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--native-threads', type=int, choices=(1, 2), default=2)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)
            or (ROOT / '.local' / args.name).exists()
            or len(args.recipient_wallet) != 6
            or len(set(p.resolve() for p in args.recipient_wallet)) != 6):
        raise RuntimeError('Fresh ignored run and six distinct recipient wallets required')
    source_check(args.notes)
    notes = json.loads(private_read(args.notes))
    if len(notes['notes']) != 4:
        raise RuntimeError('Four canonical accepted notes required')
    initial = tree_at_tip()
    receivers = json.loads(private_read(args.receivers))
    for wallet, receiver in zip(args.recipient_wallet, receivers):
        info = json.loads(subprocess.check_output([str(ROOT / 'target/release/examples/regtest_wallet'),
            'init', str(wallet)], cwd=ROOT, env=ENV, text=True, timeout=60))
        if info['receiver'] != receiver['receiver']:
            raise RuntimeError('Recipient actor differs from certified receiver opening')
        if not (wallet / 'nu7-birthday.json').exists():
            private_write(wallet / 'nu7-birthday.json', initial)
    started = time.monotonic()
    command = [str(NATIVE), '--nu7-funded-memo-full', str(args.escrow), str(args.notes), str(args.book),
               str(args.receivers), str(args.admission), args.name]
    construction = guarded_json_call(command,
        cwd=ROOT, env={**ENV, 'RAYON_NUM_THREADS': str(args.native_threads)}, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=10 * 1024 ** 3)
    output = ROOT / '.local' / args.name
    private_write(output / 'construction-summary.json', construction)
    txid = construction['native']['txid']
    audited = call('regtest-admission', 'state', args.admission, txid)
    if any(s['phase'] != 'Prepared' or not s['reserved_transaction_matches']
           or s['accepted_tickets'] != 4 for s in audited['operators']):
        raise RuntimeError('Actual funded transaction is not reserved in every journal')
    # Proving took minutes; the spent notes' anchor must still be canonical.
    source_check(args.notes)
    accepted = send(output / 'funded-nu7-transaction.bin')
    if accepted['txid'] != txid:
        raise RuntimeError('Network accepted another transaction identity')
    broadcast_record = call('regtest-admission', 'record', args.admission, txid, 'broadcast')
    print(json.dumps({'broadcast': accepted}), flush=True)
    tx = wait_confirmations(txid)
    amounts = json.loads(private_read(output / 'nu7-funded-request.json'))['amounts']
    scans = []
    for wallet, amount in zip(args.recipient_wallet, amounts):
        scan, path, _ = capture(wallet)
        openings = json.loads(private_read(path))
        if (scan['decrypted_unspent_native_notes'] != 1 or scan['balance_zatoshi'] != amount
                or len(openings['notes']) != 1
                or bytes(openings['notes'][0]['txid'])[::-1].hex() != txid
                or openings['notes'][0]['amount'] != amount):
            raise RuntimeError('Recipient scan does not show this exact auction payment')
        scans.append(scan)
    escrow_scan, remaining_path, _ = capture(args.escrow)
    if (escrow_scan['balance_zatoshi'] != 0 or escrow_scan['spent_native_notes'] != 4
            or json.loads(private_read(remaining_path))['notes']):
        raise RuntimeError('Escrow did not consume exactly all four accepted deposits')
    finalize_record = call('regtest-admission', 'record', args.admission, txid, 'finalize')
    finalized = call('regtest-admission', 'state', args.admission, txid)
    if any(s['phase'] != 'Finalized' or not s['reserved_transaction_matches'] for s in finalized['operators']):
        raise RuntimeError('Funded journals failed to finalize exact same transaction')
    summary = {'scope': 'actual public-testnet NU7 funded certified memo auction; all actors on one host',
        'construction': construction, 'full_node_accepted': True, 'canonical_confirmations': tx['confirmations'],
        'settlement_block': {'height': tx['height'], 'hash': tx['block_hash']},
        'recipient_scans': scans, 'escrow_scan': escrow_scan, 'broadcast_journals': broadcast_record,
        'finalized_journals': finalize_record, 'same_transaction_all_stages': True,
        'seconds': time.monotonic() - started, 'public_testnet': True, 'native_rayon_threads': args.native_threads,
        'finality_scope': 'two public-testnet confirmations seen through a trusted lightwalletd provider'}
    private_write(output / 'confirmed-summary.json', summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
