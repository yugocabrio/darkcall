#!/usr/bin/env python3
"""Replay actual NU7 final-book certificate under separate trusted authority.

Checks eight substitutions without reading keys or editing operator journals.
Signatures attest admission only; they are not public ZK funding evidence.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
from regtest_nu7_funding import ROOT, SDK
from regtest_deposits import private_read, private_write, ENV


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('admission', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 60
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh simple ignored check name required')
    out = ROOT/'.local'/args.name
    out.mkdir(mode=0o700)
    start = time.monotonic()
    files = ['final-book-certificate.json','manifest.json'] + [f'private-node-{i}.jsonl' for i in range(3)]
    hashes = {name:hashlib.sha256(private_read(args.admission/name)).hexdigest() for name in files}
    certificate = json.loads(private_read(args.admission/files[0]))
    authority = json.loads(private_read(args.admission/files[1]))

    def verify(cert_path, authority_path):
        return subprocess.run([str(SDK),'regtest-admission','verify',str(cert_path),str(authority_path)],
            cwd=ROOT,env=ENV,capture_output=True,timeout=60)

    result = verify(args.admission/files[0],args.admission/files[1])
    if result.returncode: raise RuntimeError('Actual final-book certificate replay failed')
    verified = json.loads(result.stdout)
    labels = ('signature','duplicate-endorsement','root','ticket','refund-receiver',
              'authority-key','authority-cutoff','authority-context')
    for label in labels:
        case = out/label
        case.mkdir(mode=0o700)
        cert = json.loads(json.dumps(certificate))
        manifest = json.loads(json.dumps(authority))
        if label == 'signature':
            signature = bytearray.fromhex(cert['endorsements'][0]['signature'])
            signature[0] ^= 1
            cert['endorsements'][0]['signature'] = signature.hex()
        elif label == 'duplicate-endorsement': cert['endorsements'][1] = cert['endorsements'][0]
        elif label in ('root','ticket','refund-receiver'):
            for e in cert['endorsements']:
                if label == 'root': e['book']['accepted_root'][31] ^= 1
                elif label == 'ticket': e['book']['entries'][0]['ticket'][0] ^= 1
                else: e['book']['entries'][0]['refund_receiver_commitment'][31] ^= 1
        elif label == 'authority-key': manifest['operator_keys'][0][0] ^= 1
        elif label == 'authority-cutoff': manifest['closes_at_utc'] += 1
        else: manifest['context']['conditions_hash'][0] ^= 1
        private_write(case/files[0],cert)
        private_write(case/files[1],manifest)
        result = verify(case/files[0],case/files[1])
        if result.returncode == 0 or b'panicked at' in result.stderr:
            raise RuntimeError('Final-book substitution not safely rejected: '+label)
    if any(hashlib.sha256(private_read(args.admission/name)).hexdigest() != hashes[name] for name in files):
        raise RuntimeError('Original certificate/authority/journals changed')
    summary = {'scope':'actual NU7 admission certificate replay; caller-trusted local authority',
        'verified':verified,'rejected':labels,'original_journals_unchanged':True,
        'signing_or_broadcast':False,'seconds':time.monotonic()-start}
    private_write(out/'summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__ == '__main__': main()
