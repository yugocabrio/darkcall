#!/usr/bin/env python3
"""Replay PUBLIC certificates/proofs against an EXPLICIT trusted test manifest.

Use synthetic_certificate for the offline admission fixture. Funding provenance
is not proved by the application endorsements. Never automatically trust keys
provided by a proof bundle. No private funding journals are copied or read.
"""
import argparse
import json
import pathlib
import shutil
from replay_public_bundle import copy_bundle, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    parser.add_argument('certificate', type=pathlib.Path)
    parser.add_argument('trusted_manifest', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    source, dest = copy_bundle(args.source, args.name)
    cert_path = dest / 'final-book-certificate.json'
    shutil.copyfile(args.certificate, cert_path)
    authority = args.trusted_manifest.resolve()
    summary = {'source': source.name, 'directory': dest.name,
               'public_only_replay': run(dest, authority=authority),
               'trusted_manifest_supplied_explicitly': True,
               'funding_journals_or_private_openings_copied': False,
               'chain_funding_or_finality_verified': False, 'tampering': {}}
    original = cert_path.read_bytes()
    try:
        cert = json.loads(original)
        cert['endorsements'][0]['signature'] = '00' * 64
        cert_path.write_text(json.dumps(cert) + '\n')
        summary['tampering']['operator_signature'] = run(dest, 'certified MPC acceptance', authority)
        cert = json.loads(original)
        cert['endorsements'][1] = cert['endorsements'][0]
        cert_path.write_text(json.dumps(cert) + '\n')
        summary['tampering']['duplicate_operator'] = run(dest, 'certified MPC acceptance', authority)
        cert = json.loads(original)
        for endorsement in cert['endorsements']:
            endorsement['book']['entries'][0]['intent_digest'][0] ^= 1
        cert_path.write_text(json.dumps(cert) + '\n')
        summary['tampering']['accepted_intent'] = run(dest, 'certified MPC acceptance', authority)
    finally:
        cert_path.write_bytes(original)
    input_path = dest / 'mpc-public/public-input0'
    public = input_path.read_bytes()
    try:
        for name, index in [('bid_commitment', 164), ('accepted_root', 168), ('refund_receiver', 169)]:
            changed = bytearray(public)
            changed[index * 32 + 31] ^= 1
            input_path.write_bytes(changed)
            summary['tampering'][name] = run(dest, 'certified MPC acceptance', authority)
    finally:
        input_path.write_bytes(public)
    changed_authority = json.loads(authority.read_text())
    changed_authority['closes_at_utc'] += 1
    wrong_path = dest / 'altered-manifest.json'
    wrong_path.write_text(json.dumps(changed_authority) + '\n')
    summary['tampering']['trusted_auction_cutoff'] = run(dest, 'certified MPC acceptance', wrong_path)
    wrong_path.unlink()
    summary['passed'] = True
    (dest / 'replay-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
