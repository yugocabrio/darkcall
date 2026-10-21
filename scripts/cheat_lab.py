#!/usr/bin/env python3
"""Cheat lab: make one dishonest change to the recorded settlement and see which check refuses it.

Each cheat copies the committed public bundle, makes the change a corrupt
operator might try, re-seals the copy's manifest so the packaging digests
match, and runs the same fixed verifiers as the settlement check: the receipt
(winner-flag) proof with the pinned Barretenberg verifier, then the full NU7
verifier (native transaction, auction proof, six output bindings, signed
accepted book). Nothing is simulated; the answer is whatever those verifiers
return. No key, pad or private opening is read.

A cheat counts as refused only when the check it targets refuses it: the
signed book for `book`, the auction proofs for `price`, the output bindings
for `payout`. Any other failure (a missing tool, an unexpected verifier
error) is reported as a test error, never as a refusal. The `none` control
runs the same pipeline on an unchanged copy, which must be accepted.
"""
import argparse
import hashlib
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
    'book': "Change the third accepted bid's commitment in all three operators' signed books",
    'price': 'Claim a clearing price one zatoshi higher, in both auction statements',
    'payout': 'Swap the output-binding proofs of the first two refunds, so each is checked against the other output',
}
CONTROL = 'none'
# The check each cheat targets, and which claim each verifier failure
# breaks, by the verifiers' own error text.
EXPECTED = {'book': 'book', 'price': 'price', 'payout': 'payouts'}
CAUGHT_BY = [('auction proof rejected', 'price'), ('row proof', 'payouts'),
             ('final book', 'book'), ('accepted root', 'book'), ('endorsement', 'book'),
             ('native effects', 'transaction'), ('transaction signature', 'transaction')]


def short(value):
    return value if len(value) <= 20 else value[:10] + '…' + value[-8:]


def forge(bundle, authority, kind, made):
    """Make the change in a working copy; return it and what exactly changed."""
    frozen = snapshot(bundle, authority, 'cheat-input')
    made.append(frozen)
    changed = None
    if kind == 'price':
        for name, index in PRICE_AT.items():
            data = bytearray(private_read(frozen / name))
            value = int.from_bytes(data[index * 32:(index + 1) * 32], 'big')
            data[index * 32:(index + 1) * 32] = (value + 1).to_bytes(32, 'big')
            write_existing(frozen / name, bytes(data))
        changed = {'files': list(PRICE_AT), 'field': 'clearing price: public input 181 of the auction statement '
                   'and 177 of the receipt statement', 'before': str(value), 'after': str(value + 1)}
    elif kind == 'payout':
        first, second = frozen / 'native-binding-proof-0', frozen / 'native-binding-proof-1'
        a, b = private_read(first), private_read(second)
        write_existing(first, b)
        write_existing(second, a)
        digest = lambda data: short(hashlib.sha256(data).hexdigest())
        changed = {'files': ['native-binding-proof-0', 'native-binding-proof-1'],
                   'field': 'Halo2 proofs of payout rows 0 and 1 (the first two refunds), swapped',
                   'before': f'row 0 {digest(a)}, row 1 {digest(b)}', 'after': f'row 0 {digest(b)}, row 1 {digest(a)}'}
    elif kind == CONTROL:
        pass
    elif kind == 'book':
        path = frozen / 'final-book-certificate.json'
        certificate = json.loads(private_read(path, 131072))
        before = bytes(certificate['endorsements'][0]['book']['entries'][2]['bid_commitment']).hex()
        for endorsement in certificate['endorsements']:
            entry = endorsement['book']['entries'][2]
            entry['bid_commitment'] = [entry['bid_commitment'][0] ^ 1] + entry['bid_commitment'][1:]
        write_existing(path, json.dumps(certificate).encode())
        after = bytes(certificate['endorsements'][0]['book']['entries'][2]['bid_commitment']).hex()
        changed = {'files': ['final-book-certificate.json'], 'ticket_index': 2,
                   'field': "its bid commitment, in all three operators' signed books "
                   '(the signatures are left as they were)',
                   'before': short('0x' + before), 'after': short('0x' + after)}
    else:
        raise ValueError('Unknown cheat')
    forged = ROOT / '.local' / (frozen.name.replace('cheat-input', 'cheat-' + kind))
    made.append(forged)
    # export() re-seals the copy's bundle.json with the new file digests, so the
    # package check passes and any refusal comes from the verifiers themselves.
    export(frozen, forged)
    return forged, changed


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
    change = CHEATS.get(kind, 'No change: the control copy must be accepted')
    forged, changed = forge(bundle, authority, kind, made)
    try:
        target = snapshot(forged, authority, 'cheat-verify')
    except RuntimeError as error:
        return outcome(kind, change, started, error=str(error), changed=changed)
    made.append(target)
    receipt = subprocess.run(['python3', str(ROOT / 'scripts/verify_external.py'), str(target / 'receipt-proof-public'),
                              '--circuit', 'delivery_flags4', '--output-label', 'cheat-flags'],
                             cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=300)
    # A receipt-proof refusal counts only when bb itself rejected the proof.
    log = target / 'receipt-proof-public/cheat-flags/valid-proof.log'
    receipt_refused = (receipt.returncode != 0 and 'valid-proof unexpected verifier exit' in receipt.stderr
                       and log.is_file() and 'Proof verification failed' in log.read_text(errors='replace'))
    full = subprocess.run([str(SDK), 'verify-funded-auction', str(target), str(target / 'trusted-authority.json')],
                          cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=900)
    errors = [line.split('Error:', 1)[1].strip().strip('"') for line in full.stderr.splitlines() if 'Error:' in line]
    explained = {claim for message in errors for text, claim in CAUGHT_BY if text in message.lower()}
    caught = explained | ({'price'} if receipt_refused else set())
    unexplained = ((receipt.returncode != 0 and not receipt_refused)
                   or (full.returncode != 0 and not explained))
    # The page shows the first message, never one that names a local path.
    shown = [message for message in errors if '/' not in message]
    return outcome(kind, change, started, caught=sorted(caught),
                   accepted=receipt.returncode == 0 and full.returncode == 0,
                   error='a verifier failed for a reason this test does not target' if unexplained else None,
                   receipt_proof_rejected=receipt_refused, full_verification_rejected=full.returncode != 0, changed=changed,
                   verifier_message=shown[0] if shown else None)


def outcome(kind, change, started, caught=(), accepted=False, error=None, **details):
    """'refused' only when the targeted check refused; the control must be 'accepted'."""
    if kind == CONTROL:
        result = 'accepted' if accepted and not error else 'error'
    elif error is None and EXPECTED[kind] in caught:
        result = 'refused'
    else:
        result = 'accepted' if accepted else 'error'
    return {'cheat': kind, 'change': change, 'expected_check': EXPECTED.get(kind), 'outcome': result,
            'refused': result == 'refused', 'caught_by': list(caught), 'test_error': error, **details,
            'seconds': round(time.monotonic() - started, 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cheats', nargs='+', choices=[CONTROL, *sorted(CHEATS)])
    parser.add_argument('--bundle', type=pathlib.Path, default=ROOT / 'demo/artifacts/testnet-auction-nu7-v2')
    parser.add_argument('--authority', type=pathlib.Path, default=ROOT / 'demo/authority/testnet-auction-nu7-v2.json')
    args = parser.parse_args()
    for kind in args.cheats:
        print(json.dumps(run(args.bundle, args.authority, kind)), flush=True)


if __name__ == '__main__':
    main()
