#!/usr/bin/env python3
"""One certified funded NU7 memo auction through actual local confirmations.

Owned isolated node only, fixture actors, no public-chain/mainnet funds. Never
resets a journal or substitutes a transaction after interrupted authorization.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ENV, private_read, private_write
from regtest_funding import tree
from regtest_nu7_funding import ROOT, SDK, capture
from regtest_nu7_node import rpc, status
from regtest_nu7_transfer import verify_source
from regtest_resources import guarded_json_call

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
    parser.add_argument('--native-threads',type=int,choices=(1,2),default=2,help='Bound native prover Rayon threads; same circuits and verifiers')
    parser.add_argument('--resume-prepared',type=pathlib.Path,help='Resume the exact unchanged prepared PCZT in a fresh checkpoint')
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)
            or (ROOT / '.local' / args.name).exists()
            or len(args.recipient_wallet) != 6
            or len(set(p.resolve() for p in args.recipient_wallet)) != 6):
        raise RuntimeError('Fresh ignored run and six distinct recipient wallets required')
    snapshot = status()
    notes = json.loads(private_read(args.notes))
    verify_source(notes, snapshot)
    if len(notes['notes']) != 4:
        raise RuntimeError('Four canonical accepted notes required')
    initial = tree(rpc('z_gettreestate', [snapshot['tip']]))
    birthdays = []
    receivers = json.loads(private_read(args.receivers))
    for wallet, receiver in zip(args.recipient_wallet, receivers):
        # Initial local wallet command is read-only for existing fixtures; the
        # native preparer/prover never loads recipient spending/viewing keys.
        info = json.loads(subprocess.check_output([str(ROOT / 'target/release/examples/regtest_wallet'),
            'init', str(wallet)], cwd=ROOT, env=ENV, text=True, timeout=60))
        if info['receiver'] != receiver['receiver']:
            raise RuntimeError('Recipient actor differs from certified receiver opening')
        path = wallet / 'nu7-birthday.json'
        if path.exists():
            birthday = json.loads(private_read(path))
        else:
            birthday = initial
            private_write(path, birthday)
        birthdays.append(birthday)
    started = time.monotonic()
    if args.resume_prepared:
        original=json.loads(private_read(args.resume_prepared / 'nu7-funded-request.json'))
        for key in ('escrow','notes','receivers','admission'):
            if pathlib.Path(original[key]).resolve()!=getattr(args,key).resolve():
                raise RuntimeError('Prepared recovery differs from original funding/recipient/admission inputs')
        command=[str(NATIVE),'--nu7-funded-memo-resume',str(args.book),str(args.resume_prepared),args.name]
    else:
        command=[str(NATIVE),'--nu7-funded-memo-full',str(args.escrow),str(args.notes),str(args.book),
                 str(args.receivers),str(args.admission),args.name]
    construction = guarded_json_call(command,
        cwd=ROOT, env={**ENV,'RAYON_NUM_THREADS':str(args.native_threads)}, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=10 * 1024 ** 3)
    output = ROOT / '.local' / args.name
    private_write(output / 'construction-summary.json', construction)
    txid = construction['native']['txid']
    audited = call('regtest-admission', 'state', args.admission, txid)
    if any(s['phase'] != 'Prepared' or not s['reserved_transaction_matches']
           or s['accepted_tickets'] != 4 for s in audited['operators']):
        raise RuntimeError('Actual funded transaction is not reserved in every journal')
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed during proving; preserve signed bytes and reconcile')
    verify_source(notes, snapshot)
    accepted = rpc('sendrawtransaction', [private_read(output / 'funded-nu7-transaction.bin').hex()], timeout=180)
    if accepted != txid:
        raise RuntimeError('Node accepted another transaction identity')
    broadcast_record = call('regtest-admission', 'record', args.admission, txid, 'broadcast')
    rpc('generate', [2], timeout=300)
    tx = rpc('getrawtransaction', [txid, 1])
    if not tx['in_active_chain'] or tx['confirmations'] < 2:
        raise RuntimeError('Funded auction lacks two canonical controlled confirmations')
    final = status()
    amounts = json.loads(private_read(output / 'nu7-funded-request.json'))['amounts']
    scans = []
    for wallet, birthday, amount in zip(args.recipient_wallet, birthdays, amounts):
        scan, path = capture(wallet, birthday, final)
        openings = json.loads(private_read(path))
        if (scan['decrypted_unspent_native_notes'] != 1 or scan['balance_zatoshi'] != amount
                or len(openings['notes']) != 1
                or bytes(openings['notes'][0]['txid'])[::-1].hex() != txid
                or openings['notes'][0]['amount'] != amount):
            raise RuntimeError('Recipient scan does not show this exact auction payment')
        scans.append(scan)
    escrow_birthday = json.loads(private_read(args.escrow / 'nu7-birthday.json'))
    escrow_scan, remaining_path = capture(args.escrow, escrow_birthday, final)
    if (escrow_scan['balance_zatoshi'] != 0 or escrow_scan['spent_native_notes'] != 4
            or json.loads(private_read(remaining_path))['notes']):
        raise RuntimeError('Escrow did not consume exactly all four accepted deposits')
    if status()['tip'] != final['tip']:
        raise RuntimeError('Chain changed during exact wallet reconciliation')
    finalize_record = call('regtest-admission', 'record', args.admission, txid, 'finalize')
    finalized = call('regtest-admission', 'state', args.admission, txid)
    if any(s['phase'] != 'Finalized' or not s['reserved_transaction_matches'] for s in finalized['operators']):
        raise RuntimeError('Funded journals failed to finalize exact same transaction')
    summary = {'scope': 'actual isolated NU7 funded certified memo auction; all actors on one host',
        'construction': construction, 'full_node_accepted': True, 'canonical_confirmations': tx['confirmations'],
        'recipient_scans': scans, 'escrow_scan': escrow_scan, 'broadcast_journals': broadcast_record,
        'finalized_journals': finalize_record, 'same_transaction_all_stages': True,
        'seconds': time.monotonic() - started, 'public_testnet': False, 'native_rayon_threads':args.native_threads,
        'finality_scope': 'two controlled local confirmations; not public-chain finality',
        'public_only_bundle_replay_and_ticket_client_pending': True}
    private_write(output / 'confirmed-summary.json', summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
