#!/usr/bin/env python3
"""Four actual NU7 deposits on PUBLIC TESTNET, client checks and signed receipts.

The testnet counterpart of regtest_nu7_deposits.py with the same SDK builder,
admission and certificate. What changes is the chain: blocks arrive every
~25 s, so "the tip did not move" becomes "our snapshot is still canonical", and
mining two blocks becomes waiting for two real confirmations. Testnet coins
only; one local test bank deposits for every ticket. Bids are never opened.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from testnet_chain import SDK, capture, send, source_check, tree_at_tip, wait_confirmations

NATIVE = ROOT / 'target/release/darkcall-native-gate'
WALLET = ROOT / 'target/release/examples/regtest_wallet'


def call(binary, *args, timeout=600):
    return json.loads(subprocess.check_output([str(binary), *map(str, args)], cwd=ROOT,
                      text=True, env=ENV, timeout=timeout))


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_wallet', type=pathlib.Path)
    parser.add_argument('private_notes', type=pathlib.Path)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    # Admission must finish before the declared funding cutoff. Regtest mines on
    # demand and uses 120 s; real testnet confirmations need a wider window.
    parser.add_argument('--funding-window', type=int, default=600)
    args = parser.parse_args()
    if not 300 <= args.funding_window <= 1800:
        raise RuntimeError('Funding window must be 5 to 30 minutes')
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected a fresh simple local run name')
    out = ROOT / '.local' / args.name
    inputs = ROOT / '.local' / (args.name + '-inputs')
    admission = ROOT / '.local' / (args.name + '-admission')
    if any(p.exists() for p in (out, inputs, admission)):
        raise RuntimeError('Preserve prior runs and choose a fresh name')
    source_check(args.private_notes)
    notes = json.loads(private_read(args.private_notes))
    bank = call(WALLET, 'init', args.source_wallet)
    source_index = next(i for i, n in enumerate(notes['notes'])
                        if n['recipient'] in [bank['receiver'], bank['internal_receiver']])
    inputs.mkdir(mode=0o700)
    book = json.loads(private_read(args.book_run / 'book.json'))
    private_write(inputs / 'private-book.json', book)
    receivers = []
    for i in range(6):
        actor = call(WALLET, 'init', ROOT / '.local' / (args.name + f'-receiver-{i}'))
        nonce = bytearray(os.urandom(32))
        nonce[0] &= 0x1f  # Canonical BN254 nonce, 253 bits of entropy.
        receivers.append({'receiver': actor['receiver'], 'nonce': list(nonce)})
    private_write(inputs / 'private-receivers.json', receivers)
    escrow = ROOT / '.local' / (args.name + '-escrow')
    call(NATIVE, '--regtest-dkg-init', escrow.name)
    # The escrow birthday is a canonical frontier taken before any deposit exists.
    initial = tree_at_tip()
    private_write(escrow / 'nu7-birthday.json', initial)
    cutoff = int(time.time()) + args.funding_window
    private_write(inputs / 'declared-cutoff.json', {'closes_at_utc': cutoff})
    construction = call(SDK, 'regtest-deposits', args.source_wallet, args.private_notes,
                        escrow, inputs / 'private-book.json', inputs / 'private-receivers.json', args.name)
    mapping = json.loads(private_read(out / 'private-deposit-map.json'))
    if mapping['source_index'] != source_index or mapping.get('network_genesis') != notes['genesis']:
        raise RuntimeError('Native builder selected another source note or network')
    private_write(out / 'private-book.json', book)
    source_check(args.private_notes)  # Anchor and source still canonical right before broadcast.
    accepted = send(out / 'nu7-regtest-transaction.bin')
    if accepted['txid'] != construction['txid']:
        raise RuntimeError('Network accepted another deposit identity')
    print(json.dumps({'broadcast': accepted}), flush=True)
    tx = wait_confirmations(accepted['txid'])
    scan, escrow_notes, final = capture(escrow)
    required = book['budget'] + book['processing_fee']
    if scan['decrypted_unspent_native_notes'] != 4 or scan['balance_zatoshi'] != 4 * required:
        raise RuntimeError('Escrow did not receive four exact deposits')
    source_check(escrow_notes)
    openings = json.loads(private_read(escrow_notes))
    for note in openings['notes']:
        if bytes(note['txid'])[::-1].hex() != accepted['txid']:
            raise RuntimeError('Escrow note came from another deposit transaction')
    admission_result = call(SDK, 'regtest-admission', 'admit', escrow, escrow_notes, out, args.book_run, admission, cutoff)
    summary = {'scope': 'actual public-testnet NU7 four-deposit admission; local test bank and clients',
        'native_construction': construction, 'full_node_accepted': True,
        'canonical_confirmations': tx['confirmations'], 'deposit_block': {'height': tx['height'], 'hash': tx['block_hash']},
        'escrow_scan': scan, 'admission': admission_result, 'final_book_signed': False,
        'funded_auction_settlement_broadcast': False, 'private_files_committed': False, 'public_testnet': True}
    private_write(out / 'confirmed-summary.json', summary)
    print(json.dumps(summary, indent=2), flush=True)
    while time.time() < cutoff:
        time.sleep(min(1, cutoff - time.time()))
    # The chain moved on during the window; certify only if the same four
    # deposits are still unspent and canonical at the new tip.
    rescan, renotes, _ = capture(escrow)
    if (rescan['decrypted_unspent_native_notes'] != 4 or rescan['balance_zatoshi'] != 4 * required
            or sorted(n['cmx'] for n in json.loads(private_read(renotes))['notes'])
            != sorted(n['cmx'] for n in openings['notes'])):
        raise RuntimeError('Escrow deposits changed before freeze; preserve state and reconcile')
    source_check(renotes)
    certified = call(SDK, 'regtest-admission', 'freeze', admission)
    summary['final_book_signed'] = True
    summary['certificate'] = certified
    private_write(out / 'certified-summary.json', summary)
    print(json.dumps(certified, indent=2), flush=True)


if __name__ == '__main__':
    main()
