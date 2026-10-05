#!/usr/bin/env python3
"""Freeze/replay public NU7 native bytes and one working-circuit row proof.

No private openings/PCZT/keys copied or read. This compatibility gate has synthetic
funding and no auction admission proof, broadcast, inclusion or finality claim.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, NATIVE, private_read, private_write, call
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('export_run', type=pathlib.Path)
    parser.add_argument('binding_run', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--shared-layout', action='store_true',
                        help='Explicit shared18 fixed trusted circuit; original layout remains the default')
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected a fresh simple ignored public replay name')
    local = (ROOT / '.local').resolve()
    export, binding = args.export_run.resolve(strict=True), args.binding_run.resolve(strict=True)
    if any(not p.is_relative_to(local) for p in (export, binding)):
        raise RuntimeError('Expected ignored compatibility fixture directories')
    target = local / args.name
    target.mkdir(mode=0o700)
    proof_file = 'shared-native-binding-proof' if args.shared_layout else 'native-binding-proof'
    row_file = 'shared-row-public.json' if args.shared_layout else 'nu7-row-public.json'
    for source, filename, bound in ((export, 'synthetic-nu7-transaction.bin', 100_000),
                                   (export, 'public-output-map.json', 16384),
                                   (binding, proof_file, 65536),
                                   (binding, row_file, 16384)):
        write_bytes(target / filename, private_read(source / filename, bound))
    started = time.monotonic()
    native = call(SDK, 'verify-export', target, target / row_file)
    mode = '--verify-nu7-shared-output-binding' if args.shared_layout else '--verify-nu7-output-binding'
    row = guarded_json_call([str(NATIVE), mode, str(target)],
        cwd=ROOT, env=ENV, timeout=900, observations=local / (args.name + '-resources.jsonl'))
    tampering = {}
    original = private_read(target / row_file, 16384)
    for field in ('cmx', 'action_index', 'txid'):
        changed = json.loads(original)
        statement = changed['statement'] if args.shared_layout else changed
        if field == 'cmx':
            statement[field][0] ^= 1
        elif field == 'action_index':
            statement[field] = 1 - statement[field]
        else:
            statement[field] = ('1' if statement[field][0] == '0' else '0') + statement[field][1:]
        # Only modify this freshly created public copy; restore exact bytes.
        path = target / row_file
        try:
            fd = os.open(path, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
            with os.fdopen(fd, 'wb') as handle:
                handle.write(json.dumps(changed).encode())
                handle.flush()
                os.fsync(handle.fileno())
            check = subprocess.run([str(SDK), 'verify-export', str(target), str(path)],
                cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=60)
            if check.returncode == 0 or 'binding row differs from actual NU7 native output' not in check.stderr:
                raise RuntimeError('Changed public native mapping was not rejected')
            tampering[field] = {'rejected': True, 'reason': 'binding row differs from actual serialized native output'}
        finally:
            fd = os.open(path, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
            with os.fdopen(fd, 'wb') as handle:
                handle.write(original)
                handle.flush()
                os.fsync(handle.fileno())
    summary = {'scope': 'public-only native NU7 transaction and single explicitly selected row binding; synthetic funding',
        'shared_layout': args.shared_layout,
        'passed': True, 'native': native, 'row': row, 'mapping_tampering': tampering,
        'public_files_copied': 4, 'private_openings_keys_pczt_copied': False,
        'seconds': time.monotonic() - started, 'auction_plan_connected': False,
        'chain_funding_or_finality_verified': False, 'broadcast': False}
    private_write(local / (args.name + '-summary.json'), summary)
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == '__main__':
    main()
