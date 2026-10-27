#!/usr/bin/env python3
"""Settle one certified fresh browser book on the isolated NU7 regtest.

The SAME sealed tickets, commitments, shares and accepted root go through the
existing actual pipeline: four real deposits from a local test wallet, chain
admission that re-verifies every client proof/pair, a funded final-book
certificate, the funded six-output settlement (fresh REP3 delivery, six output
bindings, DKG FROST, Zebra, confirmations, scans), public bundle verification
and own-FVK records for later per-ticket receipt opening.

No new cryptography and no weaker checks: this only orchestrates the existing
drivers. Bids are never read here. The local helper already saw the typed demo
bids when it created their proofs; deposits come from one local test bank.
"""
import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import threading
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_nu7_funding import SDK, capture
from regtest_nu7_node import status
from regtest_prepared_gate import write_bytes
from funded_public_bundle import export, verify

BANK = ROOT / '.local/nu7-bank-20261005'
# The funded settlement driver refuses to start proving below this headroom;
# check it before any deposit so a refusal never strands funded tickets.
MIN_START_DISK = 10 * 1024 ** 3
TERMS = {'budget': 100, 'processing_fee': 15000}
# Every executable the existing funded drivers invoke; refuse before funding
# instead of discovering a missing tool after deposits are already made.
REQUIRED = [ROOT / 'target/release' / n for n in ('darkcall-native-gate', 'darkcall-input-tool', 'poseidon_batch',
    'examples/regtest_wallet')] + [SDK, ROOT / '.deps/co-snarks/target-aes-native/release/co-noir',
    ROOT / '.deps/barretenberg-src/barretenberg/cpp/build-darkcall/bin/bb', ROOT / '.deps/noir-1.0.0-rc.2/nargo']
# Ordered stderr markers of the existing funded settlement driver. They only
# drive progress labels; success still comes from the driver's own checks.
SETTLEMENT_MARKERS = [
    ('Committed private refund delivery started', 'mpc_delivery'),
    ('Integrated native row key', 'binding_key'),
    *[(f'Integrated native row {i} proof started', f'binding_row_{i}') for i in range(6)],
    ('Integrated committed-input MPC verification started', 'mpc_verification'),
]


class Chain:
    """Where deposits and the settlement go. Regtest is the owned local node;
    testnet is standard public testnet through the pinned SDK's provider access.
    Both run the same builders, admission, native gate and public verifier."""

    def __init__(self, name):
        if name not in ('regtest', 'testnet'):
            raise RuntimeError('Expected regtest or testnet')
        self.name = name
        if name == 'testnet':
            import testnet_chain
            self.backend = testnet_chain
            self.bank = testnet_chain.BANK
        else:
            self.backend = None
            self.bank = BANK
        self.deposits = ROOT / 'scripts' / f'{name}_nu7_deposits.py'
        self.settlement = ROOT / 'scripts' / f'{name}_nu7_settlement.py'

    def status(self):
        return self.backend.status() if self.backend else status()

    def capture(self, wallet, snapshot):
        """Scan WALLET from its birthday; regtest pins the given snapshot, testnet
        scans to the current tip and checks the provider's frontier there."""
        if self.backend:
            scan, notes, _ = self.backend.capture(wallet)
            return scan, notes
        return capture(wallet, json.loads(private_read(wallet / 'nu7-birthday.json')), snapshot)

    def spendable(self, wallet, snapshot):
        """The bank's notes to deposit from; testnet may reuse its latest scan."""
        if self.backend:
            return self.backend.bank_notes(wallet)
        return self.capture(wallet, snapshot)

    def birthday(self, snapshot):
        """Frontier at SNAPSHOT's tip: where a wallet that has not seen this
        auction's payouts starts scanning."""
        if self.backend:
            return self.backend.tree_at_tip()
        from regtest_funding import tree
        from regtest_nu7_node import rpc
        return tree(rpc('z_gettreestate', [snapshot['tip']]))

    def broadcast(self, transaction, txid, notes):
        """Send the signed bytes and wait for two confirmations (mined on regtest)."""
        if self.backend:
            self.backend.source_check(notes)
            if self.backend.send(transaction)['txid'] != txid:
                raise RuntimeError('Network accepted another transaction identity')
            record = self.backend.wait_confirmations(txid)
            return {'confirmations': record['confirmations'], 'height': record['height']}
        from regtest_nu7_node import rpc
        if rpc('sendrawtransaction', [private_read(transaction).hex()], timeout=180) != txid:
            raise RuntimeError('Node accepted another transaction identity')
        rpc('generate', [2], timeout=300)
        tx = rpc('getrawtransaction', [txid, 1])
        if not tx['in_active_chain'] or tx['confirmations'] < 2:
            raise RuntimeError('Cancellation lacks two canonical controlled confirmations')
        return {'confirmations': tx['confirmations'], 'height': tx.get('height')}


REGTEST = Chain('regtest')


class Progress:
    def __init__(self, path):
        self.path = pathlib.Path(path) if path else None
        self.started = time.monotonic()
        self.lock = threading.Lock()

    def event(self, phase, state, **detail):
        value = {'phase': phase, 'state': state, 'elapsed_seconds': round(time.monotonic() - self.started, 3),
                 'at_unix': int(time.time()), **detail}
        if self.path is None:
            return
        with self.lock:
            fd = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'a') as f:
                f.write(json.dumps(value) + '\n')
                f.flush()
                os.fsync(f.fileno())


def simple(name):
    if not name or len(name) > 30 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name):
        raise RuntimeError('Expected a short fresh simple local name')
    return name


def certified_root(path):
    certificate = json.loads(private_read(path, 200000))
    book = certificate['endorsements'][0]['book']
    return bytes(book['accepted_root']).hex(), [bytes(e['ticket']).hex() for e in book['entries']]


def run_logged(command, log, progress=None, markers=()):
    """Run an existing driver; stream its output to a private log and map markers."""
    fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as handle:
        process = subprocess.Popen([str(c) for c in command], cwd=ROOT, env=ENV, text=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        seen = set()
        lines = []
        for line in process.stdout:
            handle.write(line)
            handle.flush()
            lines.append(line)
            for marker, phase in markers:
                if marker in line and phase not in seen:
                    seen.add(phase)
                    progress.event(phase, 'started')
        code = process.wait()
    if code:
        raise RuntimeError(f'{pathlib.Path(command[1]).name} stopped; private checkpoint and log retained')
    return ''.join(lines)


def summaries(text, *keys):
    """Top-level JSON objects printed by a driver that contain all KEYS."""
    decoder = json.JSONDecoder()
    values = []
    index = text.find('{')
    while index >= 0:
        try:
            value, end = decoder.raw_decode(text, index)
            if isinstance(value, dict) and all(k in value for k in keys):
                values.append(value)
            index = text.find('{', end)
        except ValueError:
            index = text.find('{', index + 1)
    if not values:
        raise RuntimeError('Driver produced no summary')
    return values


def book_run_for(auction):
    auction = auction.resolve(strict=True)
    local = (ROOT / '.local').resolve()
    if not auction.is_relative_to(local) or not auction.parent.name.startswith('browser-rehearsal-'):
        raise RuntimeError('Expected an ignored browser rehearsal auction directory')
    book_run = auction / 'clearing-book'
    book = json.loads(private_read(book_run / 'book.json', 16384))
    if (any(book[k] != v for k, v in TERMS.items()) or book['units'] not in (1, 2, 3)
            or book['reserve'] not in (0, 20) or book['active'] != [True] * 4):
        raise RuntimeError('Native settlement prototype supports 1 to 3 units, budget 100, reserve 0 or 20')
    root, tickets = certified_root(auction / 'final-certificate.json')
    return auction, book_run, root, tickets


def preflight(progress, root, phase, chain=REGTEST):
    progress.event(phase, 'started')
    missing = [p.name for p in REQUIRED if not p.is_file()]
    if missing:
        raise RuntimeError('Build the pinned tools first; missing ' + ', '.join(missing))
    if shutil.disk_usage(ROOT).free < MIN_START_DISK:
        raise RuntimeError('At least 10 GiB of free disk is required before funding a settlement')
    # Parallel payout proofs (DARKCALL_ROW_WORKERS) must pass the memory guard now,
    # not after the deposits.
    capacity = subprocess.run([str(REQUIRED[0]), '--row-capacity'], cwd=ROOT, capture_output=True, text=True, timeout=60)
    if capacity.returncode:
        raise RuntimeError('DARKCALL_ROW_WORKERS refused by the memory guard: ' + capacity.stderr.strip()[-200:])
    snapshot = chain.status()
    progress.event(phase, 'done', height=snapshot['blocks'], accepted_root=root, chain=chain.name)
    return snapshot


def fund(auction, name, progress, bank=None, chain=REGTEST):
    """Actual deposits, then chain admission and a final-book certificate over the
    SAME root and tickets as the browser book. Clearing happens afterwards."""
    auction, book_run, root, tickets = book_run_for(auction)
    funding = name + '-funding'
    if any((ROOT / '.local' / n).exists() for n in (funding, name + '-funded.json')):
        raise RuntimeError('Preserve prior runs and choose a fresh name')
    bank = bank or chain.bank
    snapshot = preflight(progress, root, 'preflight', chain)
    bank_scan, bank_notes = chain.spendable(bank, snapshot)
    if bank_scan['balance_zatoshi'] < 4 * 15100 + 25000:
        raise RuntimeError(f'Local test bank lacks {chain.name} funds')
    progress.event('funding', 'started')
    output = run_logged([sys.executable, '-B', chain.deposits,
        bank, bank_notes, book_run, '--name', funding], ROOT / '.local' / (name + '-funding.log'))
    summary = summaries(output, 'admission', 'escrow_scan')[-1]
    certified = summaries(output, 'final_book_signatures', 'receipt_quorums_reverified')[-1]
    if (not summary['full_node_accepted'] or summary['canonical_confirmations'] < 2
            or summary['escrow_scan']['decrypted_unspent_native_notes'] != 4
            or summary['admission']['client_zk_proofs_reverified'] != 4
            or summary['admission']['delivered_share_pairs_reverified'] != 12
            or not summary['admission']['accepted_root_matches']
            or certified.get('final_book_signatures') != 3):
        raise RuntimeError('Funded admission incomplete')
    admission = ROOT / '.local' / (funding + '-admission')
    funded_root, funded_tickets = certified_root(admission / 'final-book-certificate.json')
    if funded_root != root or funded_tickets != tickets:
        raise RuntimeError('Funded certificate is not the same browser book')
    public = {'deposit_txid': summary['native_construction']['txid'], 'confirmations': summary['canonical_confirmations'],
              'client_proofs': 4, 'share_pairs': 12, 'receipt_signatures': summary['admission']['operator_receipt_signatures'],
              'final_book_signatures': 3, 'same_accepted_root': True, 'accepted_root': root}
    if chain.backend:
        # Funding is done either way; a bank that keeps its old birthday reuses its latest scan.
        try:
            print(json.dumps({'bank_birthday': chain.backend.advance_birthday(bank, bank_notes, public['deposit_txid'])}),
                  file=sys.stderr, flush=True)
        except Exception as error:
            print(f'Bank birthday kept: {error}', file=sys.stderr, flush=True)
    progress.event('funding', 'done', **{k: v for k, v in public.items() if k != 'accepted_root'})
    private_write(ROOT / '.local' / (name + '-funded.json'), {'public': public, 'funding': funding,
        'auction': str(auction), 'tickets': tickets, 'chain': chain.name})
    return public


def settle_funded(auction, name, progress, threads=2, expected=None, chain=REGTEST):
    """Settle a book that fund() already funded, admitted and certified."""
    auction, book_run, root, tickets = book_run_for(auction)
    funded = json.loads(private_read(ROOT / '.local' / (name + '-funded.json')))
    funding = funded['funding']
    settlement = name + '-settlement'
    if (funded['auction'] != str(auction) or funded['public']['accepted_root'] != root
            or funded['tickets'] != tickets or funded.get('chain', 'regtest') != chain.name):
        raise RuntimeError('Funding record belongs to another book')
    if any((ROOT / '.local' / n).exists() for n in (settlement, settlement + '-public')):
        raise RuntimeError('Preserve prior runs and choose a fresh name')
    admission = ROOT / '.local' / (funding + '-admission')
    funded_root, funded_tickets = certified_root(admission / 'final-book-certificate.json')
    if funded_root != root or funded_tickets != tickets:
        raise RuntimeError('Funded certificate is not the same browser book')
    snapshot = preflight(progress, root, 'settle_preflight', chain)
    # Rescan the escrow at the current tip: other auctions may have mined blocks
    # since funding. The settlement driver requires a current snapshot.
    escrow = ROOT / '.local' / (funding + '-escrow')
    escrow_scan, escrow_notes = chain.capture(escrow, snapshot)
    if escrow_scan['decrypted_unspent_native_notes'] != 4:
        raise RuntimeError('Escrow no longer holds exactly the four admitted deposits')

    progress.event('settlement', 'started')
    wallets = []
    for i in range(6):
        wallets += ['--recipient-wallet', ROOT / '.local' / f'{funding}-receiver-{i}']
    result = summaries(run_logged([sys.executable, '-B', chain.settlement,
        escrow, escrow_notes, book_run, ROOT / '.local' / (funding + '-inputs/private-receivers.json'),
        admission, *wallets, '--name', settlement, '--native-threads', str(threads)],
        ROOT / '.local' / (name + '-settlement.log'), progress, SETTLEMENT_MARKERS),
        'construction', 'same_transaction_all_stages')[-1]
    construction = result['construction']
    txid = construction['native']['txid']
    if (not result['full_node_accepted'] or result['canonical_confirmations'] < 2
            or not result['same_transaction_all_stages'] or construction['maximum_bids_read']
            or len(result['recipient_scans']) != 6 or result['escrow_scan']['spent_native_notes'] != 4):
        raise RuntimeError('Funded settlement incomplete')
    price, sold = construction['price'], construction['sold']
    if expected is not None and (price, sold) != expected:
        raise RuntimeError('Settled clearing differs from the browser clearing')
    progress.event('settlement', 'done', txid=txid, price=price, sold=sold,
                   confirmations=result['canonical_confirmations'], recipient_scans=6,
                   seconds=round(result['seconds'], 3))

    progress.event('public_verification', 'started')
    run = ROOT / '.local' / settlement
    bundle = ROOT / '.local' / (settlement + '-public')
    export(run, bundle)
    authority = ROOT / '.local' / (settlement + '-authority.json')
    write_bytes(authority, private_read(admission / 'manifest.json', 16384))
    verified = verify(bundle, authority)
    if verified['txid'] != txid:
        raise RuntimeError('Public verification is for another transaction')
    for i in range(4):
        subprocess.run([str(SDK), 'funded-receipt-export', str(run), str(i),
                        str(ROOT / '.local' / f'{funding}-receiver-{i}')],
                       cwd=ROOT, env=ENV, check=True, capture_output=True, timeout=120)
    progress.event('public_verification', 'done', seconds=round(verified['seconds'], 3),
                   native_row_proofs_verified=verified['application']['native_row_proofs_verified'],
                   accepted_book_certified=True)
    public = {'scope': ('fresh browser book settled on public Zcash testnet' if chain.name == 'testnet'
                        else 'fresh browser book settled on isolated NU7 regtest') + '; all actors on one host',
        'accepted_root': root, 'tickets': tickets, 'same_accepted_root_and_tickets': True,
        'deposit_txid': funded['public']['deposit_txid'], 'txid': txid,
        'price': price, 'sold': sold, 'browser_clearing_matches': expected is not None,
        'confirmations': result['canonical_confirmations'], 'settlement_seconds': round(result['seconds'], 3),
        'public_verification_seconds': round(verified['seconds'], 3),
        'native_row_proofs_verified': verified['application']['native_row_proofs_verified'],
        'normal_cpp_auction_proof_verified': verified['application']['normal_cpp_auction_proof_verified'],
        'accepted_book_and_recipients_authenticated': verified['application']['accepted_book_and_recipients_authenticated'],
        'native_proof_and_signatures_verified': verified['native_proof_and_signatures_verified'],
        'transaction_bytes': verified['transaction_bytes'], 'maximum_bids_read': False,
        'deposits_from_local_test_bank': True, 'public_testnet': chain.name == 'testnet',
        'chain_inclusion_in_public_bundle': False}
    paths = {'run': str(run), 'bundle': str(bundle), 'authority': str(authority),
             'funding': funding, 'book': str(book_run / 'book.json')}
    private_write(ROOT / '.local' / (name + '-settled.json'), {'public': public, 'paths': paths})
    progress.event('complete', 'done', txid=txid)
    return public, paths


# Operator C (index 2) is the one that stopped; A and B sign the refund.
CANCEL_SIGNERS = (0, 1)
CANCEL_SCOPE = 'actual NU7 cancellation refund of a funded certified book'


def sdk_json(*args, timeout=900):
    return json.loads(subprocess.check_output([str(SDK), *map(str, args)], cwd=ROOT, env=ENV,
                                              text=True, timeout=timeout))


def cancel_funded(auction, name, progress, chain=REGTEST, signers=CANCEL_SIGNERS):
    """Refund every deposit of a funded, certified book that cannot settle, for
    example because one operator stopped (clearing needs all three). Two named
    operators sign one transaction that returns each deposit to its ticket's
    certified refund receiver, less an equal share of the network fee; the third
    operator's journal and key share are never opened. Each journal reserves one
    transaction for good, so a book reserved for this refund can no longer be
    settled, and one reserved for settlement cannot be refunded this way. There
    is no auction result, hence no auction proof: each bidder sees its refund in
    its own wallet."""
    auction, book_run, root, tickets = book_run_for(auction)
    funded = json.loads(private_read(ROOT / '.local' / (name + '-funded.json')))
    funding = funded['funding']
    if (funded['auction'] != str(auction) or funded['public']['accepted_root'] != root
            or funded['tickets'] != tickets or funded.get('chain', 'regtest') != chain.name):
        raise RuntimeError('Funding record belongs to another book')
    run = ROOT / '.local' / (name + '-cancel')
    if run.exists():
        raise RuntimeError('Preserve prior runs and choose a fresh name')
    admission = ROOT / '.local' / (funding + '-admission')
    if certified_root(admission / 'final-book-certificate.json') != (root, tickets):
        raise RuntimeError('Funded certificate is not the same browser book')
    operators = ','.join(map(str, signers))
    progress.event('cancel_preflight', 'started')
    if not SDK.is_file():
        raise RuntimeError('Build the pinned SDK first')
    snapshot = chain.status()
    escrow = ROOT / '.local' / (funding + '-escrow')
    escrow_scan, escrow_notes = chain.capture(escrow, snapshot)
    if escrow_scan['decrypted_unspent_native_notes'] != 4:
        raise RuntimeError('Escrow no longer holds exactly the four admitted deposits')
    wallets = [ROOT / '.local' / f'{funding}-receiver-{i}' for i in range(4)]
    receivers = json.loads(private_read(ROOT / '.local' / (funding + '-inputs/private-receivers.json')))
    birthday = chain.birthday(snapshot)
    for wallet, receiver in zip(wallets, receivers):
        info = json.loads(subprocess.check_output([str(ROOT / 'target/release/examples/regtest_wallet'),
            'init', str(wallet)], cwd=ROOT, env=ENV, text=True, timeout=60))
        if info['receiver'] != receiver['receiver']:
            raise RuntimeError('Refund wallet differs from the certified receiver opening')
        if not (wallet / 'nu7-birthday.json').exists():
            private_write(wallet / 'nu7-birthday.json', birthday)
    progress.event('cancel_preflight', 'done', height=snapshot['blocks'], signers=list(signers))

    progress.event('cancel_sign', 'started')
    started = time.monotonic()
    run.mkdir(mode=0o700)
    private_write(run / 'nu7-cancel-request.json', {'format': 1, 'scope': CANCEL_SCOPE,
        'sdk': '9c5705f56517c040c50f96d29f1ff15a9383f3e0', 'branch': '77190ad9', 'escrow': str(escrow),
        'notes': str(escrow_notes), 'receivers': str(ROOT / '.local' / (funding + '-inputs/private-receivers.json')),
        'admission': str(admission), 'signers': list(signers)})
    prepared = sdk_json('funded-cancel-prepare', run)
    construction = sdk_json('funded-cancel-finalize', run)
    txid = construction['txid']
    refund = construction['refund_each']
    if (prepared['txid'] != txid or construction['threshold_spends'] != 4 or construction['refunds'] != 4
            or construction['signers'] != list(signers) or construction['other_operator_key_or_journal_read']):
        raise RuntimeError('Cancellation construction incomplete')
    state = sdk_json('regtest-admission', 'state', admission, txid, operators)
    if any(s['phase'] != 'Prepared' or not s['reserved_transaction_matches'] or s['accepted_tickets'] != 4
           for s in state['operators']):
        raise RuntimeError('Refund is not reserved in both signers\' journals')
    progress.event('cancel_sign', 'done', txid=txid, refund_each=refund, signers=list(signers))

    progress.event('cancel_broadcast', 'started')
    mined = chain.broadcast(run / 'cancel-nu7-transaction.bin', txid, escrow_notes)
    sdk_json('regtest-admission', 'record', admission, txid, 'broadcast', operators)
    final = chain.status()
    scans = []
    for wallet in wallets:
        scan, notes = chain.capture(wallet, final)
        openings = json.loads(private_read(notes))
        if (scan['decrypted_unspent_native_notes'] != 1 or scan['balance_zatoshi'] != refund
                or len(openings['notes']) != 1 or openings['notes'][0]['amount'] != refund
                or bytes(openings['notes'][0]['txid'])[::-1].hex() != txid):
            raise RuntimeError('A refund wallet does not show exactly this refund')
        scans.append({'balance_zatoshi': scan['balance_zatoshi'], 'notes': 1})
    escrow_scan, remaining = chain.capture(escrow, final)
    if (escrow_scan['balance_zatoshi'] != 0 or escrow_scan['spent_native_notes'] != 4
            or json.loads(private_read(remaining))['notes']):
        raise RuntimeError('Escrow did not return exactly all four deposits')
    sdk_json('regtest-admission', 'record', admission, txid, 'finalize', operators)
    state = sdk_json('regtest-admission', 'state', admission, txid, operators)
    if any(s['phase'] != 'Finalized' or not s['reserved_transaction_matches'] for s in state['operators']):
        raise RuntimeError('Signer journals failed to finalize the refund')
    progress.event('cancel_broadcast', 'done', confirmations=mined['confirmations'])
    public = {'scope': ('funded book refunded on public Zcash testnet' if chain.name == 'testnet'
                        else 'funded book refunded on isolated NU7 regtest') + '; all actors on one host',
        'accepted_root': root, 'deposit_txid': funded['public']['deposit_txid'], 'txid': txid,
        'refunds': 4, 'refund_each': refund, 'deposit_each': 15100, 'network_fee': construction['fee'],
        'signers': ['ABC'[i] for i in signers], 'stopped_operator': 'ABC'[3 - sum(signers)],
        'stopped_operator_key_or_journal_read': False, 'threshold_spends': 4,
        'refund_wallets_checked': scans, 'escrow_empty': True,
        'confirmations': mined['confirmations'], 'seconds': round(time.monotonic() - started, 3),
        'auction_or_binding_proof': False, 'public_testnet': chain.name == 'testnet'}
    private_write(run / 'confirmed-summary.json', {'public': public, 'construction': construction})
    private_write(ROOT / '.local' / (name + '-cancelled.json'), {'public': public})
    progress.event('complete', 'done', txid=txid)
    return public


def settle(auction, name, progress, bank=None, threads=2, expected=None, chain=REGTEST):
    """Both phases back to back (CLI default)."""
    fund(auction, name, progress, bank, chain)
    return settle_funded(auction, name, progress, threads, expected, chain)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('auction', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--progress', type=pathlib.Path)
    parser.add_argument('--bank', type=pathlib.Path)
    parser.add_argument('--chain', choices=('regtest', 'testnet'), default='regtest',
                        help='testnet: standard public testnet through the pinned provider (testnet coins only)')
    parser.add_argument('--native-threads', type=int, choices=(1, 2), default=2)
    parser.add_argument('--expect', help='Browser clearing PRICE,SOLD that the settlement must reproduce')
    parser.add_argument('--phase', choices=('all', 'fund', 'settle', 'cancel'), default='all',
                        help='fund: deposits + chain admission only; settle: settle an already funded book; '
                             'cancel: refund an already funded book that cannot settle (operators A and B sign)')
    args = parser.parse_args()
    expected = tuple(int(v) for v in args.expect.split(',')) if args.expect else None
    progress = Progress(args.progress)
    chain = Chain(args.chain)
    try:
        if args.phase == 'fund':
            public = fund(args.auction, simple(args.name), progress, args.bank, chain)
        elif args.phase == 'settle':
            public, _ = settle_funded(args.auction, simple(args.name), progress, args.native_threads, expected, chain)
        elif args.phase == 'cancel':
            public = cancel_funded(args.auction, simple(args.name), progress, chain)
        else:
            public, _ = settle(args.auction, simple(args.name), progress, args.bank, args.native_threads, expected, chain)
    except BaseException as error:
        progress.event('failed', 'stopped', reason=str(error)[:200])
        raise
    print(json.dumps(public, indent=2))


if __name__ == '__main__':
    main()
