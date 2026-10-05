#!/usr/bin/env python3
"""Freeze public NU7 six-row auction artifacts and reject transaction/proof substitutions.

No private bids, delivery masks, openings, keys, PCZT or wallet copied/read.
Uses the pinned new SDK plus the two fixed compiled proof verifiers.
"""
import argparse
import json
import os
import pathlib
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call
from native_layout import read_layout, SHARED

SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'


def write_existing(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
    with os.fdopen(fd, 'wb') as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def replay(target, label, failure=None):
    return guarded_json_call([str(SDK), 'verify-auction', str(target)],
        cwd=ROOT, env=ENV, timeout=900,
        observations=ROOT / '.local' / (label + '-resources.jsonl'),
        expected_failure=failure,
        failure_log=ROOT / '.local' / (label + '-rejection.log') if failure else None)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected fresh simple ignored public run name')
    local = (ROOT / '.local').resolve()
    source = args.source.resolve(strict=True)
    if not source.is_relative_to(local) or read_layout(source) != SHARED:
        raise RuntimeError('Expected actual NU7 shared18 auction run')
    target = local / args.name
    target.mkdir(mode=0o700)
    names = [('synthetic-nu7-transaction.bin', 100000), ('nu7-native-identity.json', 16384),
             ('native-public.json', 32768), ('native-layout.json', 4096)]
    names += [(f'native-binding-proof-{i}', 65536) for i in range(6)]
    for filename, limit in names:
        write_bytes(target / filename, private_read(source / filename, limit))
    mpc = target / 'mpc-public'
    mpc.mkdir(mode=0o700)
    result = json.loads(private_read(source / 'mpc-result.json', 4096))
    original = pathlib.Path(result['run']).resolve(strict=True)
    if not original.is_relative_to(local):
        raise RuntimeError('MPC source outside ignored state')
    for filename in ('vk', 'proof.0', 'public-input0', 'tampered-price', 'tampered-plan', 'tampered-row'):
        write_bytes(mpc / filename, private_read(original / filename, 1048576))
    # Do not carry operator/private result metadata into the public bundle.
    private_write(target / 'mpc-result.json', {'run': str(mpc), 'hash_rule': result['hash_rule']})
    start = time.monotonic()
    checked = replay(target, args.name + '-pass')
    rejected = {}
    cases = [('trailing-byte', 'synthetic-nu7-transaction.bin', 'trailing data'),
             ('binding-signature', 'synthetic-nu7-transaction.bin', 'native proof or signatures invalid'),
             ('native-identity', 'nu7-native-identity.json', 'NU7 public effects identity mismatch'),
             ('action-map', 'native-public.json', 'NU7 actual native output/action substitution'),
             ('row-hash', 'native-public.json', 'NU7 MPC/actual transaction mismatch'),
             ('uploaded-vk', 'native-layout.json', 'NU7 fixed native layout metadata'),
             ('row-proof', 'native-binding-proof-0', 'NU7 fixed native row proof rejection'),
             ('auction-proof', 'mpc-public/proof.0', 'NU7 fixed C++ auction proof rejected')]
    for label, filename, failure in cases:
        path = target / filename
        data = private_read(path)
        try:
            if label == 'trailing-byte':
                altered = data + b'\0'
            elif label in ('binding-signature', 'row-proof', 'auction-proof'):
                altered = data[:-1] + bytes([data[-1] ^ 1])
            else:
                changed = json.loads(data)
                if label == 'native-identity':
                    changed['effects_digest'][0] ^= 1
                elif label == 'action-map':
                    changed['action_indices'][0] = changed['action_indices'][1]
                elif label == 'row-hash':
                    changed['row_hashes'][0][0] ^= 1
                else:
                    changed['vk'] = 'untrusted-uploaded-key'
                altered = json.dumps(changed).encode()
            write_existing(path, altered)
            rejected[label] = replay(target, args.name + '-' + label, failure)
        finally:
            write_existing(path, data)
    summary = {'scope': 'public-only NU7 six-output auction replay; synthetic funding',
               'passed': True, 'replay': checked, 'tampering': rejected,
               'private_openings_keys_pczt_delivery_copied': False,
               'public_files_copied': len(names) + 7,
               'chain_funding_or_inclusion_verified': False,
               'seconds': time.monotonic() - start}
    private_write(local / (args.name + '-summary.json'), summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
