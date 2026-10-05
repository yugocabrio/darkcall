#!/usr/bin/env python3
"""Certified four-funded auction -> private native LOCAL regtest settlement.

Maximum bids/actor spending keys are not read by this orchestrator. Recipients
scan their own keys in separate wallet processes. No public chain is contacted.
"""
import argparse
import json
import pathlib
import subprocess
from regtest_node import ROOT, rpc, status
from regtest_deposits import (ADMIT, NATIVE, ENV, private_write, call, verify_snapshot,
                             verify_note, capture)
from regtest_funding import tree
from regtest_resources import guarded_json_call


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('deposit_run', type=pathlib.Path)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
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
    snapshot = status()
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
    construction = guarded_json_call(
        [str(NATIVE), '--regtest-settlement', *map(str, (escrow, notes_path, args.book_run,
                                                       receivers_path, admission, args.name))],
        cwd=ROOT, env=ENV, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'))
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
    final = status()
    mapping = json.loads((directory / 'private-output-map.json').read_text())
    recipient_results = []
    for i in range(6):
        wallet = deposit.parent / (name + f'-receiver-{i}')
        scan, _ = capture(wallet, initial, final)
        if scan['decrypted_unspent_native_notes'] != 1 or scan['balance_zatoshi'] != mapping['amounts'][i]:
            raise RuntimeError('Recipient did not receive exact proved native settlement')
        recipient_results.append({'role': 'refund' if i < 4 else 'seller' if i == 4 else 'operator',
                                 'exact_private_amount_verified': True,
                                 'native_note_decrypted': True, 'native_path_matches_node': True})
    # Rescan escrow from its original birthday, including deposits and settlement;
    # its four owned notes must now be spent by actual canonical nullifiers.
    birthday = json.loads((escrow / f'node-capture-{notes["tip_height"]}.json').read_text())['initial']
    spent_scan, _ = capture(escrow, birthday, final)
    if (spent_scan['decrypted_unspent_native_notes'] != 0 or spent_scan['balance_zatoshi'] != 0
            or spent_scan['spent_native_notes'] != 4):
        raise RuntimeError('Escrow scanner did not confirm consumption of all four inputs')
    if status()['tip'] != final['tip']:
        raise RuntimeError('Chain changed during recipient/funding reconciliation')
    call(ADMIT, 'record', admission, accepted, 'finalize')
    summary = {'scope': 'actual isolated regtest certified four-funded MPC/native auction',
        'construction': construction, 'substituted_receiver_rejected_before_proving': True,
        'full_node_accepted': True, 'canonical_confirmations': confirmed['confirmations'],
        'recipients': recipient_results, 'all_six_exact_private_amounts_verified': True,
        'escrow_actual_spent_notes': spent_scan['spent_native_notes'],
        'three_durable_journals_finalized': True, 'maximum_bids_read': False,
        'public_only_replay_pending': True,
        'finality_scope': 'two controlled-regtest confirmations, not public-chain finality',
        'setup': construction['setup'],
        'escrow_master_key_read': construction['escrow_master_key_read'],
        'escrow_full_ask_derived_or_reconstructed': construction['escrow_full_ask_derived_or_reconstructed'],
        'independent_custody_hosts': construction['independent_custody_hosts'],
        'private_witnesses_or_keys_committed': False}
    private_write(directory / 'confirmed-summary.json', summary)
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == '__main__':
    main()
