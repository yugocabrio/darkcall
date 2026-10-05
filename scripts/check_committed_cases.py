#!/usr/bin/env python3
"""Compare actual committed REP3 proofs/private v2 delivery with a Rust oracle.

All bids here are fabricated correctness fixtures. The independent oracle knows
them; it is outside MPC/signing. This is not independent-host deployment or
malicious MPC security, nor a native/chain settlement test.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_resources import guarded_json_call


def cases():
    fixed = [('cutoff-tie', 20, [50, 50, 50, 50]),
             ('undersubscribed', 20, [90, 10, 0, 0]),
             ('exact-demand', 20, [100, 20, 20, 19]),
             ('no-qualified', 20, [19, 19, 0, 0]),
             ('budget-boundary', 20, [100, 100, 100, 100]),
             ('free-tie', 0, [0, 0, 0, 0])]
    # Deterministic fixture RNG only; protocol shares/pads continue using CSPRNGs.
    state = 0xDA4C011
    for index in range(3):
        bids = []
        for _ in range(4):
            state = (1664525 * state + 1013904223) & 0xffffffff
            bids.append(state % 101)
        fixed.append((f'generated-{index}', 0 if index == 2 else 20, bids))
    return fixed


def run(name):
    os.umask(0o077)
    if not name or len(name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name):
        raise RuntimeError('Expected fresh simple ignored case run name')
    directory = ROOT / '.local' / name
    directory.mkdir(mode=0o700)
    started = time.monotonic()
    results = []

    def execute(label, command, timeout=600):
        with (directory / (label + '.log')).open('xb') as log:
            result = subprocess.run([str(v) for v in command], cwd=ROOT, env=ENV,
                                    stdout=log, stderr=subprocess.STDOUT, timeout=timeout)
        if result.returncode:
            raise RuntimeError('Committed case failed; inspect ignored log: ' + label)

    for label, reserve, bids in cases():
        start = time.monotonic()
        oracle = json.loads(subprocess.check_output([str(ROOT / 'target/release/examples/mpc_oracle'),
                        str(reserve), *map(str, bids)], cwd=ROOT, env=ENV, text=True, timeout=30))
        book_file = directory / (label + '-book.json')
        execute(label + '-auction', [sys.executable, ROOT / 'scripts/run_mpc_gate.py',
                '--bids', *bids, '--reserve', reserve, '--result-file', book_file])
        book = pathlib.Path(json.loads(private_read(book_file, 4096))['run']).resolve(strict=True)
        if not book.is_relative_to((ROOT / '.local').resolve()):
            raise RuntimeError('Book outside ignored fixtures')
        summary = json.loads(private_read(book / 'summary.json', 65536))
        if summary['result'] != {'price': oracle['price'], 'sold': oracle['sold']}:
            raise RuntimeError('Committed auction differs from Rust oracle')
        execute(label + '-auction-cpp', [sys.executable, ROOT / 'scripts/verify_external.py', book])
        delivery_file = directory / (label + '-delivery.json')
        execute(label + '-flags', [sys.executable, ROOT / 'scripts/run_delivery_gate.py',
                '--book-run', book, '--winner-flags', '--result-file', delivery_file])
        delivery = pathlib.Path(json.loads(private_read(delivery_file, 4096))['run']).resolve(strict=True)
        if not delivery.is_relative_to((ROOT / '.local').resolve()):
            raise RuntimeError('Delivery outside ignored fixtures')
        execute(label + '-flags-cpp', [sys.executable, ROOT / 'scripts/verify_external.py',
                delivery, '--circuit', 'delivery_flags4'])
        opened = json.loads(private_read(delivery / 'delivered-amounts.json', 4096))
        for key in ('price', 'sold', 'amounts', 'winners', 'network_fee'):
            if opened[key] != oracle[key]:
                raise RuntimeError('Verified delivery differs from Rust oracle: ' + key)
        results.append({'case': label, 'synthetic_bids': bids, 'reserve': reserve,
                        'price': oracle['price'], 'sold': oracle['sold'],
                        'four_client_proofs': True, 'twelve_share_pairs': True,
                        'actual_rep3_auction_proof': True, 'actual_rep3_blinded_flags_proof': True,
                        'normal_cpp_both_verified': True,
                        'private_refunds_and_flags_match_rust': True,
                        'seconds': time.monotonic() - start})
        private_write(directory / (label + '-summary.json'), results[-1])
    summary = {'scope': 'synthetic committed-input REP3/ordinary verifier/Rust correctness regression',
               'passed': True, 'cases': results, 'independent_hosts': False,
               'native_or_chain_settlement': False, 'malicious_mpc_security': False,
               'maximum_bids_reconstructed_in_mpc': False,
               'plaintext_oracle_knows_synthetic_bids': True,
               'seconds': time.monotonic() - started}
    private_write(directory / 'summary.json', summary)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', required=True)
    parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker:
        print(json.dumps(run(args.name), indent=2))
    else:
        if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
            raise RuntimeError('Expected fresh simple ignored case run name')
        summary = guarded_json_call([sys.executable, __file__, '--name', args.name, '--worker'],
            cwd=ROOT, env=ENV, timeout=1800,
            observations=ROOT / '.local' / (args.name + '-resources.jsonl'))
        print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
