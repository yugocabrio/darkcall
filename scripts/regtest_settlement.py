#!/usr/bin/env python3
"""Certified four-funded auction -> private native LOCAL regtest settlement.

Maximum bids/actor spending keys are not read by this orchestrator. Recipients
scan their own keys in separate wallet processes. No public chain is contacted.
"""
import argparse
import json
import pathlib
import subprocess
import time
from regtest_node import ROOT, rpc, status
from regtest_deposits import (ADMIT, NATIVE, ENV, private_read, private_write, call, verify_snapshot,
                             verify_note, capture)
from regtest_funding import tree
from regtest_resources import guarded_json_call
from regtest_recovery import create_checkpoint, load_checkpoint, recover_signed


def confirm_receipts(deposit, notes, directory, construction, confirmed, resumed=False,
                     reconciliation=None):
    name = deposit.name
    escrow = deposit.parent / (name + '-escrow')
    admission = deposit.parent / (name + '-admission')
    # Recipient birthday is the original funding snapshot, not the current tip.
    initial = tree(rpc('z_gettreestate', [notes['tip_hash']]))
    final = status()
    mapping = json.loads(private_read(directory / 'private-output-map.json', 16384))
    if bytes(mapping['effects'])[::-1].hex() != construction['txid']:
        raise RuntimeError('Private receiver mapping belongs to another reserved settlement')
    recipient_results = []
    for i in range(6):
        wallet = deposit.parent / (name + f'-receiver-{i}')
        scan, _ = capture(wallet, initial, final)
        if scan['decrypted_unspent_native_notes'] != 1 or scan['balance_zatoshi'] != mapping['amounts'][i]:
            raise RuntimeError('Recipient did not receive exact proved native settlement')
        recipient_results.append({'role': 'refund' if i < 4 else 'seller' if i == 4 else 'operator',
                                 'exact_private_amount_verified': True,
                                 'native_note_decrypted': True, 'native_path_matches_node': True})
    birthday = json.loads(private_read(escrow / f'node-capture-{notes["tip_height"]}.json', 100_000_000))['initial']
    spent_scan, _ = capture(escrow, birthday, final)
    if (spent_scan['decrypted_unspent_native_notes'] != 0 or spent_scan['balance_zatoshi'] != 0
            or spent_scan['spent_native_notes'] != 4):
        raise RuntimeError('Escrow scanner did not confirm consumption of all four inputs')
    if status()['tip'] != final['tip']:
        raise RuntimeError('Chain changed during recipient/funding reconciliation')
    final_record = call(ADMIT, 'record', admission, construction['txid'], 'finalize')
    summary = {'scope': 'actual isolated regtest certified four-funded MPC/native auction',
        'construction': construction, 'substituted_receiver_rejected_before_proving': True,
        'full_node_accepted': True, 'canonical_confirmations': confirmed['confirmations'],
        'recipients': recipient_results, 'all_six_exact_private_amounts_verified': True,
        'escrow_actual_spent_notes': spent_scan['spent_native_notes'],
        'three_durable_journals_finalized': True, 'maximum_bids_read': False,
        'public_only_replay_pending': not resumed,
        'finality_scope': 'two controlled-regtest confirmations, not public-chain finality',
        'setup': construction['setup'],
        'escrow_master_key_read': construction.get('escrow_master_key_read', True),
        'escrow_full_ask_derived_or_reconstructed': construction.get('escrow_full_ask_derived_or_reconstructed', True),
        'independent_custody_hosts': construction.get('independent_custody_hosts', False),
        'resumed_exact_signed_transaction': resumed, 'reconciliation': reconciliation,
        'finalize_record': final_record, 'private_witnesses_or_keys_committed': False}
    summary_path = (f'recovery-summary-{time.time_ns()}.json' if resumed else 'confirmed-summary.json')
    private_write(directory / summary_path, summary)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('deposit_run', type=pathlib.Path)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--resume-signed', action='store_true',
                        help='Reverify and reconcile the exact existing signed transaction; no reconstruction/unlocking')
    parser.add_argument('--resume-built', metavar='SOURCE_NAME',
                        help='Resume an unapproved built PCZT and verify existing row proofs while all journals remain Frozen')
    args = parser.parse_args()
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected a fresh simple local run name')
    deposit = args.deposit_run.resolve()
    if not deposit.is_relative_to((ROOT / '.local').resolve()):
        raise RuntimeError('Only ignored actual-regtest deposit runs supported')
    name = deposit.name
    escrow = deposit.parent / (name + '-escrow')
    admission = deposit.parent / (name + '-admission')
    inputs = deposit.parent / (name + '-inputs')
    if args.resume_signed:
        if args.resume_built:
            raise RuntimeError('Choose exactly one recovery stage')
        directory, construction, notes, confirmed, reconciliation = recover_signed(deposit, args.book_run, args.name)
        print(json.dumps(confirm_receipts(deposit, notes, directory, construction, confirmed,
                                         resumed=True, reconciliation=reconciliation), indent=2), flush=True)
        return
    snapshot = status()
    source = None
    if args.resume_built:
        source_checkpoint, _ = load_checkpoint(deposit, args.book_run, args.resume_built)
        if source_checkpoint['tip'] != snapshot['tip']:
            raise RuntimeError('Unapproved native checkpoint chain changed; explicit reconciliation required')
        source = ROOT / '.local' / args.resume_built
        if (source / 'regtest-transaction.bin').exists():
            raise RuntimeError('Complete signed transaction exists; use --resume-signed instead of signing again')
    notes_path = escrow / f'private-notes-{snapshot["blocks"]}.json'
    notes = json.loads(notes_path.read_text())
    initial = tree(verify_snapshot(notes, snapshot))
    for n in notes['notes']:
        verify_note(n)
    if len(notes['notes']) != 4:
        raise RuntimeError('Expected exactly four actual unspent deposits')
    receivers_path = inputs / 'private-receivers.json'
    recipients = json.loads(receivers_path.read_text())
    bad = [dict(r) for r in recipients]
    bad[0]['receiver'] = list(recipients[1]['receiver'])
    bad_path = inputs / (args.name + '-wrong-receiver.json')
    private_write(bad_path, bad)
    negative = subprocess.run([str(NATIVE), '--regtest-settlement', str(escrow), str(notes_path),
        str(args.book_run), str(bad_path), str(admission), args.name + '-reject'],
        cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=60)
    if negative.returncode == 0 or 'receiver opening differs from certified client intent' not in negative.stderr:
        raise RuntimeError('Substituted receiver was not rejected before proving/signing')
    bad_path.unlink()
    create_checkpoint(deposit, args.book_run, args.name, snapshot)
    command = [str(NATIVE), '--regtest-resume-built' if source else '--regtest-settlement',
               *map(str, (escrow, notes_path, args.book_run, receivers_path, admission, args.name))]
    if source:
        command.append(str(source))
    construction = guarded_json_call(command,
        cwd=ROOT, env=ENV, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=16 * 1024 ** 3)
    directory = ROOT / '.local' / args.name
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed during proving; rescan/reconcile before broadcast')
    verify_snapshot(notes, snapshot)
    for n in notes['notes']:
        verify_note(n)
    binary = (directory / 'regtest-transaction.bin').read_bytes()
    accepted = rpc('sendrawtransaction', [binary.hex()], timeout=180)
    if accepted != construction['txid']:
        raise RuntimeError('Node accepted another settlement identity')
    call(ADMIT, 'record', admission, accepted, 'broadcast')
    rpc('generate', [2], timeout=300)
    confirmed = rpc('getrawtransaction', [accepted, 1])
    if not confirmed['in_active_chain'] or confirmed['confirmations'] < 2:
        raise RuntimeError('Settlement not canonical with two controlled confirmations')
    print(json.dumps(confirm_receipts(deposit, notes, directory, construction, confirmed), indent=2), flush=True)


if __name__ == '__main__':
    main()
