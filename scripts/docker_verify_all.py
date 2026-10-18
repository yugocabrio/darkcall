#!/usr/bin/env python3
"""Container entry: preflight, then verify every committed funded bundle.

Each bundle is checked with its separately committed authority by the same
fixed verifiers as ./verify.sh --funded, then a corrupted copy of each bundle's
receipt proof must be refused. Prints one public JSON summary.
"""
import json
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
BUNDLES = [('browser-auction-nu7-v1', 'auction created and settled by clicking through the browser UI, Oct 18'),
           ('fresh-book-nu7-v1', 'browser-created book settled from the CLI driver (resumed), Oct 18'),
           ('canonical-nu7-funded-v1', 'recorded Oct 5 settlement with fixture bids')]


def main():
    sys.path.insert(0, str(ROOT / 'scripts'))
    from funded_public_bundle import verify, snapshot, export
    from public_bundle import verify_flags
    from replay_nu7_auction import write_existing
    from regtest_deposits import private_read
    (ROOT / '.local').mkdir(mode=0o700, exist_ok=True)
    results = []
    for name, scope in BUNDLES:
        bundle = ROOT / 'demo/artifacts' / name
        authority = ROOT / 'demo/authority' / (name + '.json')
        started = time.monotonic()
        result = verify(bundle, authority)
        # Negative check: recompute packaging hashes for a changed receipt proof;
        # only the fixed cryptographic verifier can refuse it.
        frozen = snapshot(bundle, authority, 'docker-tamper-input')
        proof = frozen / 'receipt-proof-public/proof.0'
        data = private_read(proof)
        write_existing(proof, bytes([data[0] ^ 1]) + data[1:])
        forged = ROOT / '.local' / ('docker-forged-' + name)
        export(frozen, forged)
        flags = snapshot(forged, authority, 'docker-forged') / 'receipt-proof-public'
        try:
            verify_flags(flags, 'docker-forged')
            refused = False
        except RuntimeError:
            # Accept only the fixed verifier's cryptographic rejection, not a crash.
            refused = b'Proof verification failed' in (flags / 'docker-forged/valid-proof.log').read_bytes()
        if not refused:
            raise RuntimeError('Corrupted receipt proof was not cryptographically refused')
        results.append({'bundle': name, 'scope': scope, 'txid': result['txid'],
            'native_proof_and_signatures_verified': result['native_proof_and_signatures_verified'],
            'native_row_proofs_verified': result['application']['native_row_proofs_verified'],
            'normal_cpp_auction_proof_verified': result['application']['normal_cpp_auction_proof_verified'],
            'accepted_book_and_recipients_authenticated': result['application']['accepted_book_and_recipients_authenticated'],
            'receipt_fixed_proof_verified': result['receipt_fixed_proof_verified'],
            'corrupted_receipt_proof_refused': refused,
            'private_material_read': result['private_openings_keys_pczt_read'],
            'seconds': round(time.monotonic() - started, 3)})
    uname = subprocess.check_output(['uname', '-srm'], text=True).strip()
    print(json.dumps({'environment': 'clean Linux container built from git archive; ' + uname,
                      'results': results, 'all_passed': True}, indent=2))


if __name__ == '__main__':
    main()
