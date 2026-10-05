#!/usr/bin/env python3
"""Refuse altered funded preparation inputs on ignored private copies only."""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import time
from regtest_deposits import ENV, private_read, private_write
from regtest_nu7_funding import ROOT, SDK
from regtest_nu7_prepare import journal_hashes


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 60
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh ignored check name required')
    source = args.prepared.resolve()
    request = json.loads(private_read(source / 'nu7-funded-request.json'))
    admission = pathlib.Path(request['admission'])
    before = journal_hashes(admission)
    original_pczt = hashlib.sha256(private_read(source / 'nu7-prepared.pczt')).hexdigest()
    out = ROOT / '.local' / args.name
    out.mkdir(mode=0o700)
    start = time.monotonic()
    labels = ('request-branch', 'request-extra', 'request-amount', 'receiver-substitution',
              'source-witness', 'source-branch', 'source-duplicate', 'certificate-signature',
              'missing-journal', 'receipt-memo', 'receipt-book')
    for label in labels:
        case = out / label
        case.mkdir(mode=0o700)
        # No custody packages, receiver keys or source journal is copied here.
        for name in ('receipt-proof-public', 'receipt-book.json', 'nu7-auction-request.json',
                     'ticket-0.memo', 'ticket-1.memo', 'ticket-2.memo', 'ticket-3.memo'):
            p = source / name
            if p.is_dir():
                shutil.copytree(p, case / name)
            else:
                shutil.copyfile(p, case / name)
                (case / name).chmod(0o600)
        req = json.loads(json.dumps(request))
        receivers = json.loads(private_read(pathlib.Path(req['receivers'])))
        notes = json.loads(private_read(pathlib.Path(req['notes'])))
        local_admission = case / 'admission'
        local_admission.mkdir(mode=0o700)
        for name in ('manifest.json', 'final-book-certificate.json',
                     'private-node-0.jsonl', 'private-node-1.jsonl', 'private-node-2.jsonl'):
            shutil.copyfile(admission / name, local_admission / name)
            (local_admission / name).chmod(0o600)
        req['admission'] = str(local_admission)
        req['receivers'] = str(case / 'receivers.json')
        req['notes'] = str(case / 'notes.json')
        if label == 'request-branch': req['branch'] = '37a5165b'
        elif label == 'request-extra': req['permit_unsigned'] = True
        elif label == 'request-amount': req['amounts'][0] += 1
        elif label == 'receiver-substitution': receivers[0] = receivers[1]
        elif label == 'source-witness': notes['notes'][0]['auth_path'][0][0] ^= 1
        elif label == 'source-branch': notes['branch'] = '37a5165b'
        elif label == 'source-duplicate': notes['notes'][1] = notes['notes'][0]
        elif label == 'certificate-signature':
            cert = json.loads(private_read(local_admission / 'final-book-certificate.json'))
            sig = bytearray.fromhex(cert['endorsements'][0]['signature'])
            sig[0] ^= 1
            cert['endorsements'][0]['signature'] = sig.hex()
            (local_admission / 'final-book-certificate.json').unlink()
            private_write(local_admission / 'final-book-certificate.json', cert)
        elif label == 'missing-journal': (local_admission / 'private-node-1.jsonl').unlink()
        elif label == 'receipt-memo':
            memo = bytearray(private_read(case / 'ticket-0.memo'))
            memo[70] ^= 1
            (case / 'ticket-0.memo').write_bytes(memo)
        elif label == 'receipt-book':
            book = json.loads(private_read(case / 'receipt-book.json'))
            book['ids'][0][0] ^= 1
            (case / 'receipt-book.json').unlink()
            private_write(case / 'receipt-book.json', book)
        private_write(case / 'nu7-funded-request.json', req)
        private_write(case / 'receivers.json', receivers)
        private_write(case / 'notes.json', notes)
        result = subprocess.run([str(SDK), 'prepare-funded-auction', str(case)],
            cwd=ROOT, env=ENV, capture_output=True, timeout=60)
        private_write(case / 'stderr.json', {'stderr': result.stderr.decode('utf-8', errors='replace')})
        if (result.returncode == 0 or b'panicked at' in result.stderr
                or (case / 'nu7-prepared.pczt').exists()):
            raise RuntimeError('Funded preparation substitution not safely rejected: ' + label)
    result = subprocess.run([str(SDK), 'finalize-auction', str(source)],
        cwd=ROOT, env=ENV, capture_output=True, timeout=30)
    if result.returncode == 0 or b'NU7 auction request identity or value conservation' not in result.stderr:
        raise RuntimeError('Synthetic finalizer accepted actual preparation scope')
    if (journal_hashes(admission) != before
            or hashlib.sha256(private_read(source / 'nu7-prepared.pczt')).hexdigest() != original_pczt):
        raise RuntimeError('Source changed during negative preparation checks')
    summary = {'scope': 'funded NU7 preparation substitutions; no signing or chain inclusion',
        'rejected': labels, 'synthetic_finalizer_refuses_actual_scope': True,
        'source_pczt_and_journals_unchanged': True, 'seconds': time.monotonic() - start}
    private_write(out / 'summary.json', summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
