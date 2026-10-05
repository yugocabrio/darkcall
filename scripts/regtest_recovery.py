"""Reconcile an EXACT already-signed certified settlement on the owned regtest.

No notes are unlocked, no new effects/signatures are created and no root/cutoff
is changed. Missing unsigned-PCZT or reorg/expiry recovery is deliberately not
implemented by this path. Network failures must never mean 'not broadcast'.
"""
import hashlib
import json
import pathlib
from regtest_node import ROOT, GENESIS, BRANCH, RpcError, rpc, status
from regtest_deposits import (ADMIT, NATIVE, call, private_read, private_write,
                             verify_snapshot, verify_note)


def checkpoint_path(name):
    if not name or len(name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name):
        raise RuntimeError('Expected a simple existing local settlement name')
    return ROOT / '.local' / (name + '-recovery.json')


def inputs(deposit, height):
    prefix = deposit.name
    return {
        'notes': deposit.parent / (prefix + '-escrow') / f'private-notes-{height}.json',
        'receivers': deposit.parent / (prefix + '-inputs') / 'private-receivers.json',
        'manifest': deposit.parent / (prefix + '-admission') / 'manifest.json',
        'certificate': deposit.parent / (prefix + '-admission') / 'final-book-certificate.json',
    }


def local_directory(path):
    path = pathlib.Path(path).resolve(strict=True)
    if not path.is_dir() or path.parent != (ROOT / '.local').resolve():
        raise RuntimeError('Expected an existing direct ignored local directory')
    return path


def create_checkpoint(deposit, book, name, snapshot):
    deposit, book = local_directory(deposit), local_directory(book)
    if snapshot['genesis'] != GENESIS or snapshot['consensus']['chaintip'] != BRANCH:
        raise RuntimeError('Wrong recovery checkpoint network')
    paths = inputs(deposit, snapshot['blocks'])
    value = {'format': 1, 'genesis': GENESIS, 'branch': BRANCH,
             'deposit_run': deposit.name, 'book_run': book.name,
             'height': snapshot['blocks'], 'tip': snapshot['tip'],
             'input_sha256': {k: hashlib.sha256(private_read(p)).hexdigest() for k, p in paths.items()}}
    private_write(checkpoint_path(name), value)
    return value


def load_checkpoint(deposit, book, name):
    deposit, book = local_directory(deposit), local_directory(book)
    value = json.loads(private_read(checkpoint_path(name), 16384))
    if (set(value) != {'format', 'genesis', 'branch', 'deposit_run', 'book_run',
                      'height', 'tip', 'input_sha256'} or value['format'] != 1
            or value['genesis'] != GENESIS or value['branch'] != BRANCH
            or value['deposit_run'] != deposit.name or value['book_run'] != book.name
            or type(value['height']) is not int or value['height'] < 2):
        raise RuntimeError('Recovery checkpoint differs from requested auction/network')
    paths = inputs(deposit, value['height'])
    expected = {k: hashlib.sha256(private_read(p)).hexdigest() for k, p in paths.items()}
    if expected != value['input_sha256']:
        raise RuntimeError('Recovery funding/recipients/authority/certificate changed')
    # Original witnesses refer to this canonical historical snapshot. The
    # settlement may already be confirmed at a later height; do not interpret
    # consumed notes as missing funding and recreate them.
    if rpc('getblockhash', [value['height']]) != value['tip']:
        raise RuntimeError('Recovery checkpoint is orphaned; explicit reorg reconciliation required')
    notes = json.loads(private_read(paths['notes']))
    verify_snapshot(notes, {'blocks': value['height'], 'tip': value['tip']})
    for note in notes['notes']:
        verify_note(note)
    return value, notes


def lookup_transaction(txid):
    try:
        return rpc('getrawtransaction', [txid, 1])
    except RpcError as error:
        if error.code != -5:
            raise
        return None


def check_transaction(observed, binary, txid):
    if (observed.get('txid') != txid or observed.get('hex') != binary.hex()
            or type(observed.get('confirmations', 0)) is not int
            or observed.get('confirmations', 0) < 0):
        raise RuntimeError('Node transaction differs from exact verified signed bytes')
    confirmations = observed.get('confirmations', 0)
    if confirmations == 0 and observed.get('blockhash'):
        raise RuntimeError('Unconfirmed transaction has a block association; reorg reconciliation required')
    if confirmations > 0 and observed.get('in_active_chain') is not True:
        raise RuntimeError('Settlement is not canonical; explicit reorg reconciliation required')
    return confirmations


def reconcile_transaction(binary, txid, admission, original_tip, operators):
    if (len(operators) != 3 or [o['operator'] for o in operators] != [0, 1, 2]
            or any(o['reserved_transaction_matches'] is not True or o['accepted_tickets'] != 4
                   or o['phase'] not in ('Prepared', 'Broadcast', 'Finalized') for o in operators)):
        raise RuntimeError('All journals must retain the same exact reserved settlement')
    chain = status()  # pinned loopback genesis/branch guard before any mutation
    observed = lookup_transaction(txid)
    confirmations = check_transaction(observed, binary, txid) if observed else 0
    if observed is not None and confirmations == 0 and txid not in rpc('getrawmempool'):
        raise RuntimeError('Unconfirmed settlement is absent from mempool; explicit reconciliation required')
    if any(o['phase'] == 'Finalized' for o in operators) and confirmations < 2:
        raise RuntimeError('Finalized settlement lost confirmations; explicit reorg reconciliation required')
    sent = False
    if observed is None:
        if chain['tip'] != original_tip:
            raise RuntimeError('Unbroadcast settlement chain changed; rescan/expiry reconciliation required')
        if rpc('sendrawtransaction', [binary.hex()], timeout=180) != txid:
            raise RuntimeError('Node accepted a different signed settlement identity')
        sent = True
    # This also completes a crash between the three journal writes. It cannot
    # substitute a digest, skip a stage or change an input reservation.
    broadcast = call(ADMIT, 'record', admission, txid, 'broadcast')
    mined = max(0, 2 - confirmations)
    if mined:
        rpc('generate', [mined], timeout=300)
    confirmed = lookup_transaction(txid)
    if confirmed is None or check_transaction(confirmed, binary, txid) < 2:
        raise RuntimeError('Exact settlement did not gain two canonical controlled confirmations')
    return confirmed, {'exact_signed_transaction_sent': sent, 'confirmation_blocks_generated': mined,
                       'broadcast_record': broadcast}


def recover_signed(deposit, book, name):
    checkpoint, notes = load_checkpoint(deposit, book, name)
    deposit = local_directory(deposit)
    directory = local_directory(ROOT / '.local' / name)
    admission = deposit.parent / (deposit.name + '-admission')
    construction = json.loads(private_read(directory / 'funded-summary.json', 16384))
    binary = private_read(directory / 'regtest-transaction.bin')
    txid = construction['txid']
    states = call(ADMIT, 'state', admission, txid)['operators']
    if any(s['reserved_transaction_matches'] is not True
           or s['phase'] not in ('Prepared', 'Broadcast', 'Finalized') for s in states):
        raise RuntimeError('Signed recovery requires exact durable reservations; unsigned PCZT resume remains separate')
    verified = call(NATIVE, '--verify-certified-integrated', directory,
                    admission / 'manifest.json', timeout=600)
    if (verified['txid'] != txid or verified['transaction_bytes'] != len(binary)
            or not verified['acceptance_or_recipient_receipts_authenticated']
            or private_read(directory / 'regtest-transaction.bin') != binary):
        raise RuntimeError('Signed recovery bytes/identity or certified proofs changed')
    confirmed, reconciliation = reconcile_transaction(binary, txid, admission, checkpoint['tip'], states)
    return directory, construction, notes, confirmed, reconciliation
