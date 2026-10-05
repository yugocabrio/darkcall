#!/usr/bin/env python3
"""Guarded synthetic four-bid/six-output NU7 MPC/native/FROST integration.

Uses the separate pinned new SDK and byte-compatible fixed shared18 proofs.
Fabricated funding, local trusted dealer, no chain/RPC/broadcast operation.
"""
import argparse
import json
import os
import pathlib
import time
from regtest_deposits import ROOT, NATIVE, ENV, private_write
from regtest_resources import guarded_json_call


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--prepare-only', action='store_true',
                        help='Verify v2 delivery and six native bytes; no binding/native proofs or signatures')
    parser.add_argument('--receipt-memos', action='store_true',
                        help='Put fixed-proof-authenticated per-ticket receipts in the four refund outputs')
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected fresh simple ignored NU7 run name')
    book = args.book_run.resolve(strict=True)
    if not book.is_relative_to((ROOT / '.local').resolve()) or not book.is_dir():
        raise RuntimeError('Expected ignored synthetic committed book')
    if (ROOT / '.local' / args.name).exists():
        raise RuntimeError('Preserve existing NU7 run; choose an unused name')
    started = time.monotonic()
    mode = ('--nu7-auction-memo-prepare-only' if args.prepare_only else '--nu7-integrated-memos-shared') if args.receipt_memos else ('--nu7-auction-prepare-only' if args.prepare_only else '--nu7-integrated-shared')
    result = guarded_json_call([str(NATIVE), mode, str(book), args.name],
        cwd=ROOT, env=ENV, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=(4 if args.prepare_only else 10) * 1024 ** 3)
    summary = {'scope': 'synthetic NU7 auction/native integration; no chain funding',
               'external_process_seconds': time.monotonic() - started, 'result': result}
    private_write(ROOT / '.local' / (args.name + '-summary.json'), summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
