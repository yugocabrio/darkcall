#!/usr/bin/env python3
"""Bound the heavy existing-circuit new-SDK output compatibility experiment.

This is synthetic and has no auction funding, RPC or broadcast. Two measured
keygen runs consumed disk headroom; refuse an identical low-capacity restart.
"""
import argparse
import json
import pathlib
from regtest_deposits import ROOT, NATIVE, ENV, private_write
from regtest_resources import guarded_json_call


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('export_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--threads', type=int, choices=(1, 2), default=2)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected a fresh simple ignored binding run name')
    export = args.export_run.resolve(strict=True)
    if not export.is_relative_to((ROOT / '.local').resolve()):
        raise RuntimeError('Expected ignored synthetic export directory')
    result = guarded_json_call([str(NATIVE), '--nu7-output-binding', str(export), args.name],
        cwd=ROOT, env={**ENV, 'RAYON_NUM_THREADS': str(args.threads)}, timeout=1800,
        observations=ROOT / '.local' / (args.name + '-resources.jsonl'),
        minimum_start_disk_bytes=16 * 1024 ** 3)
    private_write(ROOT / '.local' / (args.name + '-summary.json'), result)
    print(json.dumps(result, indent=2), flush=True)


if __name__ == '__main__':
    main()
