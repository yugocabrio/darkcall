#!/usr/bin/env python3
"""Four actual LOCAL-regtest encrypted deposits, client checks and signed receipts.

Fixture tickets/terms and local client actors; no real bids or public-chain funds.
Existing source/escrow wallets are never reset. Maximum bids are not opened here.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_node import ROOT, GENESIS, rpc, status
from regtest_funding import tree

NATIVE = ROOT / 'target/release/darkcall-native-gate'
WALLET = ROOT / 'target/release/examples/regtest_wallet'
ADMIT = ROOT / 'target/release/examples/regtest_admission'
ENV = {**os.environ, 'RAYON_NUM_THREADS': '2'}


def private_write(path, value):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as f:
        json.dump(value, f)
        f.write('\n')
        f.flush()
        os.fsync(f.fileno())
    parent = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(parent)
    finally:
        os.close(parent)


def call(binary, *args, timeout=600):
    return json.loads(subprocess.check_output([str(binary), *map(str, args)], cwd=ROOT,
                      text=True, env=ENV, timeout=timeout))


def verify_snapshot(notes, snapshot):
    if (notes['scope'] != 'isolated regtest wallet openings' or notes['genesis'] != GENESIS
            or notes['nu6_3_height'] != 2 or notes['tip_hash'] != snapshot['tip']
            or notes['tip_height'] != snapshot['blocks']):
        raise RuntimeError('Source is not the current pinned local snapshot')
    node_tree = rpc('z_gettreestate', [snapshot['tip']])
    if bytes(notes['anchor']).hex() != node_tree['ironwood']['commitments']['finalRoot']:
        raise RuntimeError('Source anchor differs from canonical full-node tree')
    return node_tree


def verify_note(note):
    block_hash = bytes(note['mined_block'])[::-1].hex()
    if rpc('getblockhash', [note['mined_height']]) != block_hash:
        raise RuntimeError('Funding block is no longer canonical')
    block = rpc('getblock', [block_hash, 2])
    txid = bytes(note['txid'])[::-1].hex()
    tx = next(t for t in block['tx'] if t['txid'] == txid)
    if tx['ironwood']['actions'][note['action_index']]['cmx'] != bytes(note['cmx']).hex():
        raise RuntimeError('Funding cmx differs from actual full-node output')
    coinbase = bool(tx['vin']) and 'coinbase' in tx['vin'][0]
    if note.get('coinbase', coinbase) != coinbase:
        raise RuntimeError('Funding coinbase metadata differs from actual transaction')
    if tx['confirmations'] < (101 if coinbase else 2):
        raise RuntimeError('Funding requires maturity or two controlled confirmations')
    return tx


def capture(wallet, initial, snapshot):
    blocks = []
    for height in range(initial['height'] + 1, snapshot['blocks'] + 1):
        if len(blocks) >= 500:
            raise RuntimeError('Capture exceeds bounded gate')
        h = rpc('getblockhash', [height])
        b = rpc('getblock', [h, 2])
        blocks.append({'height': height, 'hash': h, 'prev_hash': b['previousblockhash'],
            'time': b['time'], 'ironwood_size': b['trees']['ironwood']['size'],
            'transactions': [tx['hex'] for tx in b['tx']]})
    final = tree(rpc('z_gettreestate', [snapshot['tip']]))
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed while capturing')
    p = wallet / f'node-capture-{snapshot["blocks"]}.json'
    private_write(p, {'genesis': GENESIS, 'branch': snapshot['consensus']['chaintip'],
        'nu6_3_height': 2, 'initial': initial, 'final_tree': final, 'blocks': blocks})
    result = call(WALLET, 'scan', wallet, p)
    return result, wallet / f'private-notes-{snapshot["blocks"]}.json'


def main():
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
    notes = json.loads(args.private_notes.read_text())
    node_tree = verify_snapshot(notes, snapshot)
    bank = call(WALLET, 'init', args.source_wallet)
    source_index = next(i for i, n in enumerate(notes['notes'])
        if n['recipient'] in [bank['receiver'], bank['internal_receiver']]
        and (not n.get('coinbase', True) or snapshot['blocks'] >= n['mined_height'] + 100))
    verify_note(notes['notes'][source_index])
    inputs.mkdir(mode=0o700)
    book = json.loads((args.book_run / 'book.json').read_text())
    private_write(inputs / 'private-book.json', book)
    receivers = []
    for i in range(6):
        actor = call(WALLET, 'init', ROOT / '.local' / (args.name + f'-receiver-{i}'))
        nonce = bytearray(os.urandom(32))
        nonce[0] &= 0x1f  # Canonical BN254 nonce, 253 bits of entropy.
        receivers.append({'receiver': actor['receiver'], 'nonce': list(nonce)})
    private_write(inputs / 'private-receivers.json', receivers)
    escrow = ROOT / '.local' / (args.name + '-escrow')
    call(WALLET, 'init', escrow)
    initial = tree(node_tree)
    cutoff = int(time.time()) + 120
    private_write(inputs / 'declared-cutoff.json', {'closes_at_utc': cutoff})
    construction = call(NATIVE, '--regtest-deposits', args.source_wallet, args.private_notes,
                        escrow, inputs / 'private-book.json', inputs / 'private-receivers.json', args.name)
    mapping = json.loads((out / 'private-deposit-map.json').read_text())
    if mapping['source_index'] != source_index:
        raise RuntimeError('Native builder selected another source note')
    private_write(out / 'private-book.json', book)
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed before deposit broadcast')
    accepted = rpc('sendrawtransaction', [(out / 'regtest-transaction.bin').read_bytes().hex()], timeout=180)
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
    openings = json.loads(escrow_notes.read_text())
    verify_snapshot(openings, final)
    for note in openings['notes']:
        if bytes(note['txid'])[::-1].hex() != accepted:
            raise RuntimeError('Escrow note came from another deposit transaction')
        verify_note(note)
    admission_result = call(ADMIT, 'admit', escrow, escrow_notes, out, args.book_run, admission, cutoff)
    summary = {'scope': 'actual isolated regtest four-deposit admission; fixture clients/tickets/terms',
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
    certified = call(ADMIT, 'freeze', admission)
    summary['final_book_signed'] = True
    summary['certificate'] = certified
    private_write(out / 'certified-summary.json', summary)
    print(json.dumps(certified, indent=2), flush=True)


if __name__ == '__main__':
    main()
