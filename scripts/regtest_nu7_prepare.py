#!/usr/bin/env python3
"""Prepare the certified funded NU7 six-output memo PCZT, without authorization.

The caller checks the owned node's current canonical funding snapshot. The
existing Frozen journals remain untouched; no reservation or broadcast occurs.
"""
import argparse
import hashlib
import json
import os
import pathlib

from regtest_deposits import ENV, private_read, private_write
from regtest_nu7_node import ROOT, status
from regtest_nu7_transfer import verify_source
from regtest_resources import guarded_json_call

NATIVE = ROOT / 'target/release/darkcall-native-gate'


def journal_hashes(admission):
    return [hashlib.sha256(private_read(admission / f'private-node-{i}.jsonl')).hexdigest()
            for i in range(3)]


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('escrow', 'notes', 'book', 'receivers', 'admission'):
        parser.add_argument(name, type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)
            or (ROOT / '.local' / args.name).exists()):
        raise RuntimeError('Fresh ignored funded preparation name required')
    snapshot = status()
    notes = json.loads(private_read(args.notes))
    verify_source(notes, snapshot)
    if len(notes['notes']) != 4:
        raise RuntimeError('Exactly four accepted actual notes required')
    before = journal_hashes(args.admission)
    result = guarded_json_call([str(NATIVE), '--nu7-funded-memo-prepare',
        str(args.escrow), str(args.notes), str(args.book), str(args.receivers),
        str(args.admission), args.name], cwd=ROOT, env=ENV, timeout=600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'))
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('Chain changed during preparation; rescan before authorization')
    verify_source(notes, snapshot)
    if journal_hashes(args.admission) != before:
        raise RuntimeError('Preparation altered durable funding state')
    result['canonical_source_snapshot_checked'] = True
    result['original_frozen_journals_unchanged'] = True
    private_write(ROOT / '.local' / args.name / 'prepared-summary.json', result)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
