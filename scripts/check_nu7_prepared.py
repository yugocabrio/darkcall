#!/usr/bin/env python3
"""Attack an unsigned synthetic NU7 checkpoint before proving/FROST.

Private copies stay ignored. This checks malformed/substituted native inputs
and refusal to authorize missing application proofs, not a completed SNARK.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from replay_nu7_auction import write_existing, SDK


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected fresh simple ignored check name')
    source = args.source.resolve(strict=True)
    if not source.is_relative_to((ROOT / '.local').resolve()):
        raise RuntimeError('Expected ignored synthetic NU7 checkpoint')
    target = ROOT / '.local' / args.name
    target.mkdir(mode=0o700)
    for filename in ('nu7-auction-request.json', 'private-nu7-auction-keys.json',
                     'nu7-native-identity.json', 'nu7-prepared.pczt', 'native-layout.json'):
        write_bytes(target / filename, private_read(source / filename))
    start = time.monotonic()
    rejected = {}
    cases = [('missing-proofs', None, 'NU7 application proof rejected before FROST signing'),
             ('amount-substitution', 'nu7-auction-request.json', 'NU7 prepared output substituted'),
             ('reused-action', 'private-nu7-auction-keys.json', 'NU7 recipient index collision'),
             ('receiver-substitution', 'private-nu7-auction-keys.json', 'prepared output cannot decrypt'),
             ('effects-substitution', 'nu7-native-identity.json', 'NU7 prepared effects changed'),
             ('unknown-key-schema', 'private-nu7-auction-keys.json', 'strict NU7 auction schema rejected'),
             ('pczt-trailing-byte', 'nu7-prepared.pczt', 'NU7 PCZT parse')]
    for label, filename, failure in cases:
        path = target / filename if filename else None
        data = private_read(path) if path else None
        try:
            if path:
                if label == 'pczt-trailing-byte':
                    changed_bytes = data + b'\0'
                else:
                    changed = json.loads(data)
                    if label == 'amount-substitution':
                        changed['amounts'][0] -= 1
                        changed['amounts'][4] += 1
                    elif label == 'reused-action':
                        changed['indices'][1] = changed['indices'][0]
                    elif label == 'receiver-substitution':
                        changed['receivers'][0] = changed['receivers'][1]
                    elif label == 'effects-substitution':
                        changed['effects_digest'][0] ^= 1
                    else:
                        changed['trusted'] = True
                    changed_bytes = json.dumps(changed).encode()
                write_existing(path, changed_bytes)
            result = subprocess.run([str(SDK), 'finalize-auction', str(target)],
                cwd=ROOT, env=ENV, capture_output=True, timeout=60)
            write_bytes(target / (label + '.log'), result.stdout + result.stderr)
            if result.returncode == 0 or failure.encode() not in result.stderr:
                raise RuntimeError('Expected NU7 pre-sign refusal absent: ' + label)
            if (target / 'synthetic-nu7-transaction.bin').exists():
                raise RuntimeError('Rejected checkpoint emitted transaction')
            rejected[label] = {'rejected': True, 'expected_reason': failure}
        finally:
            if path:
                write_existing(path, data)
    summary = {'scope': 'unsigned synthetic NU7 pre-sign input attack checks; no binding proof',
               'tampering': rejected, 'passed': True,
               'native_application_proof_generated': False, 'transaction_emitted': False,
               'broadcast': False, 'seconds': time.monotonic() - start}
    private_write(ROOT / '.local' / (args.name + '-summary.json'), summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
