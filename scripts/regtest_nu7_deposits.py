#!/usr/bin/env python3
"""Four actual NU7 LOCAL-regtest encrypted deposits, client checks and signed receipts.

Fixture tickets/terms and local client actors; no real bids or public-chain funds.
Existing source/escrow wallets are never reset. Maximum bids are not opened here.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_nu7_node import ROOT, GENESIS, rpc, status
from regtest_funding import tree
from regtest_deposits import private_read, private_write, ENV
from regtest_nu7_funding import SDK, capture
from regtest_nu7_transfer import verify_source

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
    args = parser.parse_args()
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected a fresh simple local run name')
    out = ROOT / '.local' / args.name
    inputs = ROOT / '.local' / (args.name + '-inputs')
    admission = ROOT / '.local' / (args.name + '-admission')
    if any(p.exists() for p in (out, inputs, admission)):
        raise RuntimeError('Preserve prior runs and choose a fresh name')
    snapshot = status()
    notes = json.loads(private_read(args.private_notes))
    verify_source(notes, snapshot)
    node_tree = rpc('z_gettreestate', [snapshot['tip']])
    bank = call(WALLET, 'init', args.source_wallet)
    source_index = next(i for i, n in enumerate(notes['notes'])
        if n['recipient'] in [bank['receiver'], bank['internal_receiver']]
        and (not n.get('coinbase', True) or snapshot['blocks'] >= n['mined_height'] + 100))
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
    initial = tree(node_tree)
    private_write(escrow / 'nu7-birthday.json', initial)
    cutoff = int(time.time()) + 120
    private_write(inputs / 'declared-cutoff.json', {'closes_at_utc': cutoff})
    construction = call(SDK, 'regtest-deposits', args.source_wallet, args.private_notes,
                        escrow, inputs / 'private-book.json', inputs / 'private-receivers.json', args.name)
    mapping = json.loads(private_read(out / 'private-deposit-map.json'))
    if mapping['source_index'] != source_index:
        raise RuntimeError('Native builder selected another source note')
    private_write(out / 'private-book.json', book)
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed before deposit broadcast')
    verify_source(notes, snapshot)
    accepted = rpc('sendrawtransaction', [private_read(out / 'nu7-regtest-transaction.bin').hex()], timeout=180)
    if accepted != construction['txid']:
        raise RuntimeError('Node accepted another deposit identity')
    rpc('generate', [2], timeout=300)
    tx = rpc('getrawtransaction', [accepted, 1])
    if not tx['in_active_chain'] or tx['confirmations'] < 2:
        raise RuntimeError('Deposits not canonically confirmed')
    final = status()
    scan, escrow_notes = capture(escrow, initial, final)
    required = book['budget'] + book['processing_fee']
    if scan['decrypted_unspent_native_notes'] != 4 or scan['balance_zatoshi'] != 4 * required:
        raise RuntimeError('Escrow did not receive four exact deposits')
    openings = json.loads(private_read(escrow_notes))
    verify_source(openings, final)
    for note in openings['notes']:
        if bytes(note['txid'])[::-1].hex() != accepted:
            raise RuntimeError('Escrow note came from another deposit transaction')
    admission_result = call(SDK, 'regtest-admission', 'admit', escrow, escrow_notes, out, args.book_run, admission, cutoff)
    summary = {'scope': 'actual isolated NU7 regtest four-deposit admission; fixture clients/tickets/terms',
        'native_construction': construction, 'full_node_accepted': True,
        'canonical_confirmations': tx['confirmations'], 'escrow_scan': scan,
        'admission': admission_result, 'final_book_signed': False,
        'funded_auction_settlement_broadcast': False, 'private_files_committed': False}
    private_write(out / 'confirmed-summary.json', summary)
    print(json.dumps(summary, indent=2), flush=True)
    # Keep admission and cutoff certificate separate. The persistent receipt
    # files let a later invocation certify exactly the same accepted book.
    while time.time() < cutoff:
        time.sleep(min(1, cutoff - time.time()))
    if status()['tip'] != final['tip']:
        raise RuntimeError('Chain changed before freeze; rescan/reconcile before certifying')
    certified = call(SDK, 'regtest-admission', 'freeze', admission)
    summary['final_book_signed'] = True
    summary['certificate'] = certified
    private_write(out / 'certified-summary.json', summary)
    print(json.dumps(certified, indent=2), flush=True)


if __name__ == '__main__':
    main()
