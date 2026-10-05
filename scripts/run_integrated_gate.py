#!/usr/bin/env python3
"""Resource-guarded synthetic four-bid/six-output MPC + native/FROST gate.

No chain funding or broadcast. The book contains synthetic committed inputs;
the native consumer reads verified v2 refund/winner openings, never bid plaintext.
Do not run alongside another heavy native prover.
"""
import argparse
import json
import os
import pathlib
import time
from regtest_resources import guarded_json_call

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('book_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--shared-native-binding', action='store_true',
                        help='Explicit compiled shared18 circuit; historical working33 remains default')
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected fresh simple ignored run name')
    book = args.book_run.resolve(strict=True)
    if not book.is_relative_to((ROOT / '.local').resolve()) or not book.is_dir():
        raise RuntimeError('Expected ignored synthetic book directory')
    if (ROOT / '.local' / args.name).exists():
        raise RuntimeError('Integration directory already exists; preserve its checkpoint')
    started = time.monotonic()
    result = guarded_json_call(
        [str(ROOT / 'target/release/darkcall-native-gate'),
         '--integrated-shared' if args.shared_native_binding else '--integrated',
         str(book), args.name],
        cwd=ROOT, env={**os.environ, 'RAYON_NUM_THREADS': '2'}, timeout=3600,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=(10 if args.shared_native_binding else 16) * 1024 ** 3)
    summary = {'scope': 'synthetic committed four-bid native integration; no chain funding/broadcast',
               'external_process_seconds': round(time.monotonic() - started, 3),
               'native': result}
    with (ROOT / '.local' / (args.name + '-summary.json')).open('x') as output:
        json.dump(summary, output, indent=2)
        output.write('\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
