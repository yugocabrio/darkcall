#!/usr/bin/env python3
"""Offline exact-PCZT recovery from copied partial Prepare journal prefixes.

Uses historical actual-regtest notes, possibly already spent. NEVER broadcasts,
mines, or changes original journals. Not a new funded-chain settlement. Private
copies intentionally reconstruct a crash point; production journals are append-only.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
from regtest_deposits import ROOT, NATIVE, ADMIT, ENV, private_read, private_write, call
from regtest_resources import guarded_json_call


def write_bytes(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def journal_prefix(data, prepared):
    """Retain a validated original prefix, without editing the original file."""
    lines = data.splitlines(keepends=True)
    if not lines or not data.endswith(b'\n'):
        raise RuntimeError('Original journal is empty or truncated')
    prefix = [lines[0]]
    for sequence, line in enumerate(lines[1:], 1):
        record = json.loads(line)
        if record['sequence'] != sequence:
            raise RuntimeError('Original journal sequence is inconsistent')
        event = record['action']['event']
        if event == 'Prepare':
            if record['action']['cancellation']:
                raise RuntimeError('Expected an actual settlement reservation')
            if prepared:
                prefix.append(line)
            return b''.join(prefix)
        if event in ('Broadcast', 'Finalize'):
            raise RuntimeError('Original journal has no preceding Prepare')
        prefix.append(line)
    raise RuntimeError('Original journal lacks the completed-run reservation')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('deposit_run', type=pathlib.Path)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('built_source', type=pathlib.Path)
    parser.add_argument('proved_source', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 32
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected a fresh simple local run name, max 32 characters')
    local = (ROOT / '.local').resolve()
    deposit, book, built, proved = [p.resolve(strict=True) for p in
                                   (args.deposit_run, args.book_run, args.built_source, args.proved_source)]
    if any(not p.is_relative_to(local) for p in (deposit, book, built, proved)):
        raise RuntimeError('All fixtures must be ignored local directories')
    original_admission = deposit.parent / (deposit.name + '-admission')
    escrow = deposit.parent / (deposit.name + '-escrow')
    receivers = deposit.parent / (deposit.name + '-inputs') / 'private-receivers.json'
    mapping = json.loads(private_read(proved / 'private-output-map.json', 16384))
    txid = bytes(mapping['effects'])[::-1].hex()
    # The construction's immutable Python checkpoint records the exact snapshot.
    checkpoint = json.loads(private_read(built.with_name(built.name + '-recovery.json'), 16384))
    if (checkpoint['format'] != 1 or checkpoint['deposit_run'] != deposit.name
            or checkpoint['book_run'] != book.name or checkpoint['branch'] != '37a5165b'):
        raise RuntimeError('Original checkpoint does not belong to these fixtures')
    notes = escrow / f'private-notes-{checkpoint["height"]}.json'
    for key, path in {'notes': notes, 'receivers': receivers,
                      'manifest': original_admission / 'manifest.json',
                      'certificate': original_admission / 'final-book-certificate.json'}.items():
        if hashlib.sha256(private_read(path)).hexdigest() != checkpoint['input_sha256'][key]:
            raise RuntimeError('Original fixture input changed since construction')
    source = local / (args.name + '-source')
    admission = local / (args.name + '-admission')
    negative = local / (args.name + '-reject-admission')
    for path in (source, admission, negative):
        path.mkdir(mode=0o700)
    original_hashes = {}

    def copy(path, target, limit):
        data = private_read(path, limit)
        original_hashes[path] = hashlib.sha256(data).hexdigest()
        write_bytes(target, data)

    for filename, limit in [('private-built-settlement.pczt', 1_000_000),
                            ('native-public.json', 32768), ('native-private-rows.json', 16384)]:
        copy(built / filename, source / filename, limit)
    if private_read(built / 'native-private-rows.json', 16384) != private_read(proved / 'native-private-rows.json', 16384):
        raise RuntimeError('Proved rows do not match the exact original PCZT source')
    for i in range(6):
        filename = f'native-binding-proof-{i}'
        copy(proved / filename, source / filename, 65536)
    for filename in ('manifest.json', 'final-book-certificate.json'):
        copy(original_admission / filename, admission / filename, 131072)
        copy(original_admission / filename, negative / filename, 131072)
    prefixes, negative_prefixes = [], []
    for i in range(3):
        path = original_admission / f'private-node-{i}.jsonl'
        data = private_read(path)
        original_hashes[path] = hashlib.sha256(data).hexdigest()
        prefix = journal_prefix(data, prepared=i < 2)
        prefixes.append(prefix)
        write_bytes(admission / path.name, prefix)
        bad = prefix
        if i == 0:
            lines = prefix.splitlines(keepends=True)
            record = json.loads(lines[-1])
            record['action']['cancellation'] = True
            bad = b''.join(lines[:-1]) + json.dumps(record).encode() + b'\n'
        negative_prefixes.append(bad)
        write_bytes(negative / path.name, bad)

    before = call(ADMIT, 'state', admission, txid)
    operators = before['operators']
    if ([o['phase'] for o in operators] != ['Prepared', 'Prepared', 'Frozen']
            or not all(o['reserved_transaction_matches'] for o in operators[:2])):
        raise RuntimeError('Copied partial-reservation fixture did not reconstruct the expected stage')

    def command(target, name):
        return [str(NATIVE), '--regtest-resume-built', *map(str,
                (escrow, notes, book, receivers, target, name, source))]

    log = local / (args.name + '-rejection.log')
    fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as handle:
        rejected = subprocess.run(command(negative, args.name + '-reject'), cwd=ROOT,
                                  env=ENV, stdout=handle, stderr=handle, timeout=120)
        handle.flush()
        os.fsync(handle.fileno())
    if (rejected.returncode == 0 or b'reservation cannot substitute effects/purpose' not in private_read(log)):
        raise RuntimeError('Cancellation-purpose substitution did not fail before native proving/signing')
    if any((local / (args.name + '-reject') / filename).exists() for filename in
           ('private-built-settlement.pczt', 'prepared-settlement.pczt', 'regtest-transaction.bin')):
        raise RuntimeError('Rejected reservation reached native checkpoint/signing')
    for i in range(3):
        # The purpose-changed negative input must also remain exactly unchanged.
        negative_path = negative / f'private-node-{i}.jsonl'
        if private_read(negative_path) != negative_prefixes[i]:
            raise RuntimeError('Rejected operation mutated a copied journal')

    construction = guarded_json_call(command(admission, args.name), cwd=ROOT, env=ENV,
        timeout=1800, observations=local / (args.name + '-resources.jsonl'))
    if construction['txid'] != txid:
        raise RuntimeError('Native resume substituted transaction effects')
    after = call(ADMIT, 'state', admission, txid)
    if not all(o['phase'] == 'Prepared' and o['reserved_transaction_matches'] for o in after['operators']):
        raise RuntimeError('Native resume did not preserve exact reservation in all three copied journals')
    for i in range(3):
        data = private_read(admission / f'private-node-{i}.jsonl')
        if i < 2:
            if data != prefixes[i]:
                raise RuntimeError('An existing exact Prepare was rewritten or appended twice')
        else:
            records = data[len(prefixes[i]):].splitlines()
            if not data.startswith(prefixes[i]) or len(records) != 1 or json.loads(records[0])['action']['event'] != 'Prepare':
                raise RuntimeError('Frozen journal did not receive exactly one new reservation')
    for path, digest in original_hashes.items():
        if hashlib.sha256(private_read(path)).hexdigest() != digest:
            raise RuntimeError('Original construction/proofs/journals changed')
    profile = json.loads((local / args.name / 'native-proof-summary.json').read_bytes())
    summary = {'scope': 'offline historical actual-regtest note construction from copied journal prefixes; never broadcast',
        'construction': construction, 'existing_prepared_journals': 2, 'frozen_journals': 1,
        'existing_prepare_events_rewritten_or_duplicated': 0, 'new_prepare_events': 1,
        'same_transaction_effects': True, 'original_journals_and_source_unchanged': True,
        'cancellation_purpose_substitution_rejected_before_native_proving_signing': True,
        'native_proof_profile': profile, 'maximum_bids_read': False,
        'historical_notes_may_already_be_spent': True, 'chain_funding_verified_here': False,
        'broadcast': False, 'mining': False, 'rpc_called': False,
        'private_material_committed': False}
    private_write(local / (args.name + '-summary.json'), summary)
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == '__main__':
    main()
