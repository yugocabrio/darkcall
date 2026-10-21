#!/usr/bin/env python3
"""Cheat lab: make one dishonest change to the recorded settlement and see which check refuses it.

Each cheat copies the committed public bundle, makes the change a corrupt
operator might try, re-seals the copy's manifest so the packaging digests
match, and runs the same fixed verifiers as the settlement check: the receipt
(winner-flag) proof with the pinned Barretenberg verifier, then the full NU7
verifier (native transaction, auction proof, six output bindings, signed
accepted book). Nothing is simulated; the answer is whatever those verifiers
return. No key, pad or private opening is read.
"""
import argparse
import json
import pathlib
import shutil
import subprocess
import time
from funded_public_bundle import snapshot, export
from public_bundle import SDK
from regtest_deposits import ROOT, ENV, private_read
from replay_nu7_auction import write_existing

# Public-input positions of the clearing price in the two auction statements.
PRICE_AT = {'mpc-public/public-input0': 181, 'receipt-proof-public/public-input0': 177}
CHEATS = {
    'price': 'Claim a clearing price one zatoshi higher, in both auction statements',
    'payout': "Swap the output-binding proofs of the first two refunds, as if each went to the other bidder's output",
    'book': "Replace the third accepted bid's commitment in all three operators' signed books",
}
# Which claim each verifier failure breaks, by the verifiers' own error text.
CAUGHT_BY = [('auction proof rejected', 'price'), ('row proof', 'payouts'),
             ('final book', 'book'), ('accepted root', 'book'), ('endorsement', 'book'),
             ('native effects', 'transaction'), ('transaction signature', 'transaction')]


def forge(bundle, authority, kind, made):
    frozen = snapshot(bundle, authority, 'cheat-input')
    made.append(frozen)
    if kind == 'price':
        for name, index in PRICE_AT.items():
            data = bytearray(private_read(frozen / name))
            value = int.from_bytes(data[index * 32:(index + 1) * 32], 'big')
            data[index * 32:(index + 1) * 32] = (value + 1).to_bytes(32, 'big')
            write_existing(frozen / name, bytes(data))
    elif kind == 'payout':
        first, second = frozen / 'native-binding-proof-0', frozen / 'native-binding-proof-1'
        a, b = private_read(first), private_read(second)
        write_existing(first, b)
        write_existing(second, a)
    elif kind == 'book':
        path = frozen / 'final-book-certificate.json'
        certificate = json.loads(private_read(path, 131072))
        for endorsement in certificate['endorsements']:
            entry = endorsement['book']['entries'][2]
            entry['bid_commitment'] = [entry['bid_commitment'][0] ^ 1] + entry['bid_commitment'][1:]
        write_existing(path, json.dumps(certificate).encode())
    else:
        raise ValueError('Unknown cheat')
    forged = ROOT / '.local' / (frozen.name.replace('cheat-input', 'cheat-' + kind))
    made.append(forged)
    export(frozen, forged)
    return forged


def run(bundle, authority, kind):
    started = time.monotonic()
    made = []
    try:
        return check(bundle, authority, kind, made, started)
    finally:
        # The three working copies hold only public files; drop them so repeated
        # cheats on the hosted demo do not accumulate.
        for path in made:
            shutil.rmtree(path, ignore_errors=True)


def check(bundle, authority, kind, made, started):
    forged = forge(bundle, authority, kind, made)
    try:
        target = snapshot(forged, authority, 'cheat-verify')
    except RuntimeError as error:
        return {'cheat': kind, 'change': CHEATS[kind], 'refused': True, 'caught_by': ['bundle'],
                'verifier_message': str(error), 'seconds': round(time.monotonic() - started, 1)}
    made.append(target)
    receipt = subprocess.run(['python3', str(ROOT / 'scripts/verify_external.py'), str(target / 'receipt-proof-public'),
                              '--circuit', 'delivery_flags4', '--output-label', 'cheat-flags'],
                             cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=300)
    full = subprocess.run([str(SDK), 'verify-funded-auction', str(target), str(target / 'trusted-authority.json')],
                          cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=900)
    errors = [line.split('Error:', 1)[1].strip().strip('"') for line in full.stderr.splitlines() if 'Error:' in line]
    caught = sorted({claim for message in errors for text, claim in CAUGHT_BY if text in message.lower()})
    if receipt.returncode:
        caught = sorted(set(caught) | {'price'})
    # The page shows the first message, never one that names a local path.
    shown = [message for message in errors if '/' not in message]
    return {'cheat': kind, 'change': CHEATS[kind], 'refused': bool(receipt.returncode or full.returncode),
            'receipt_proof_rejected': receipt.returncode != 0, 'full_verification_rejected': full.returncode != 0,
            'caught_by': caught, 'verifier_message': shown[0] if shown else None,
            'seconds': round(time.monotonic() - started, 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cheats', nargs='+', choices=sorted(CHEATS))
    parser.add_argument('--bundle', type=pathlib.Path, default=ROOT / 'demo/artifacts/testnet-auction-nu7-v2')
    parser.add_argument('--authority', type=pathlib.Path, default=ROOT / 'demo/authority/testnet-auction-nu7-v2.json')
    args = parser.parse_args()
    for kind in args.cheats:
        print(json.dumps(run(args.bundle, args.authority, kind)), flush=True)


if __name__ == '__main__':
    main()
