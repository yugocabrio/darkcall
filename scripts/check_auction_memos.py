#!/usr/bin/env python3
"""Authenticate four ticket memos before the six-output SDK signs anything.

Prepared mode checks fixed flags proof and openings. Finalization also compares
the auction/context/price/sold statement to the independently verified six-row
settlement. No viewing or spending keys are read by this verifier.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read


def check(directory, linked=False):
    directory = pathlib.Path(directory).resolve(strict=True)
    local = (ROOT / '.local').resolve()
    if not directory.is_relative_to(local):
        raise RuntimeError('Expected ignored six-output memo checkpoint')
    request = json.loads(private_read(directory / 'nu7-auction-request.json', 4096))
    if request.get('receipt_memos') is not True or len(request['amounts']) != 6:
        raise RuntimeError('Expected explicit six-output receipt request')
    proof = directory / 'receipt-proof-public'
    statement = private_read(proof / 'public-input0', 187 * 32)
    if len(statement) != 187 * 32:
        raise RuntimeError('Expected exact blinded winner statement')
    nonce = str(time.time_ns())
    with (directory / f'receipt-fixed-{nonce}.log').open('xb') as log:
        result = subprocess.run(['python3', str(ROOT / 'scripts/verify_external.py'), str(proof),
            '--circuit', 'delivery_flags4', '--output-label', 'auction-memo-' + nonce],
            cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT, timeout=120)
    if result.returncode:
        raise RuntimeError('Fixed ticket proof rejected')
    # The producer may know all pads. The recipient command still needs only its
    # own memo and its own viewing key; this verifier never handles that key.
    outcomes = []
    for index in range(4):
        result = subprocess.run([str(ROOT / 'target/release/darkcall-input-tool'),
            'flags-ticket-memo-open', str(directory / 'receipt-book.json'), str(index),
            str(directory / f'ticket-{index}.memo'), str(proof / 'public-input0')],
            cwd=ROOT, env=ENV, capture_output=True, timeout=30)
        if result.returncode or len(result.stdout) > 4096:
            raise RuntimeError('Ticket memo opening rejected')
        outcome = json.loads(result.stdout)
        if not outcome['opening_verified'] or outcome['refund'] != request['amounts'][index]:
            raise RuntimeError('Ticket memo differs from native refund amount')
        outcomes.append(outcome)
    if linked:
        path = pathlib.Path(json.loads(private_read(directory / 'mpc-result.json', 4096))['run']).resolve(strict=True)
        if not path.is_relative_to(local):
            raise RuntimeError('Settlement proof path outside ignored checkpoints')
        settlement = private_read(path / 'public-input0', 232 * 32)
        if (len(settlement) != 232 * 32 or settlement[:169*32] != statement[:169*32]
                or settlement[181*32:183*32] != statement[177*32:179*32]):
            raise RuntimeError('Receipt proof differs from verified settlement book/price/sold')
    return {'fixed_cpp_receipt_proof_verified': True, 'four_ticket_openings_verified': True,
        'refund_amounts_match_request': True, 'settlement_statement_link_checked': linked,
        'viewing_or_spending_keys_read': False}


if __name__ == '__main__':
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=pathlib.Path)
    parser.add_argument('--linked-settlement', action='store_true')
    args = parser.parse_args()
    print(json.dumps(check(args.directory, args.linked_settlement)))
