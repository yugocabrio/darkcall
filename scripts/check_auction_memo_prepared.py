#!/usr/bin/env python3
"""Actual six-output memo checkpoint refusals before native proving/FROST.

Copies only ignored synthetic artifacts. It never edits the original PCZT,
source book or keys and never broadcasts. This is not a full six-row proof run.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from replay_nu7_auction import SDK, write_existing


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected fresh simple checkpoint name')
    source = args.source.resolve(strict=True)
    if not source.is_relative_to((ROOT / '.local').resolve()):
        raise RuntimeError('Expected ignored synthetic checkpoint')
    target = ROOT / '.local' / args.name
    target.mkdir(mode=0o700)
    (target / 'receipt-proof-public').mkdir(mode=0o700)
    names = ['nu7-auction-request.json', 'private-nu7-auction-keys.json', 'nu7-native-identity.json',
        'nu7-prepared.pczt', 'native-layout.json', 'receipt-book.json']
    names += [f'ticket-{i}.memo' for i in range(4)]
    names += ['receipt-proof-public/' + n for n in ('vk', 'proof.0', 'public-input0',
        'tampered-price', 'tampered-root', 'tampered-cipher', 'tampered-winner')]
    originals = {}
    for name in names:
        data = private_read(source / name)
        originals[name] = hashlib.sha256(data).hexdigest()
        write_bytes(target / name, data)
    cases = [
        ('missing-six-row-proofs', None, 'NU7 application proof rejected before FROST signing'),
        ('wrong-refund-memo', 'ticket-0.memo', 'auction receipt proof/opening rejected'),
        ('wrong-ticket-memo', 'ticket-1.memo', 'auction receipt proof/opening rejected'),
        ('extra-memo-data', 'ticket-2.memo', 'auction receipt proof/opening rejected'),
        ('corrupt-receipt-proof', 'receipt-proof-public/proof.0', 'auction receipt proof/opening rejected'),
        ('uploaded-receipt-vk', 'receipt-proof-public/vk', 'auction receipt proof/opening rejected'),
        ('changed-receipt-statement', 'receipt-proof-public/public-input0', 'auction receipt proof/opening rejected'),
        ('wrong-refund-request', 'nu7-auction-request.json', 'auction receipt proof/opening rejected'),
        ('disable-memo-request', 'nu7-auction-request.json', 'NU7 prepared output substituted'),
        ('wrong-receiver', 'private-nu7-auction-keys.json', 'prepared output cannot decrypt'),
        ('duplicate-index', 'private-nu7-auction-keys.json', 'NU7 recipient index collision'),
        ('changed-effects', 'nu7-native-identity.json', 'NU7 prepared effects changed'),
        ('trailing-pczt', 'nu7-prepared.pczt', 'NU7 PCZT parse')]
    start = time.monotonic()
    rejected = []
    for label, name, diagnostic in cases:
        data = private_read(target / name) if name else None
        try:
            if name:
                if name.endswith('.json'):
                    value = json.loads(data)
                    if label == 'wrong-refund-request':
                        value['amounts'][0] -= 1
                        value['amounts'][4] += 1
                    elif label == 'disable-memo-request': value['receipt_memos'] = False
                    elif label == 'wrong-receiver': value['receivers'][0] = value['receivers'][1]
                    elif label == 'duplicate-index': value['indices'][1] = value['indices'][0]
                    else: value['effects_digest'][0] ^= 1
                    changed = json.dumps(value).encode()
                else:
                    changed = bytearray(data)
                    if label == 'trailing-pczt': changed += b'\x00'
                    else: changed[{'wrong-refund-memo':104, 'wrong-ticket-memo':9,
                        'extra-memo-data':201,'changed-receipt-statement':177*32+31}.get(label,0)] ^= 1
                write_existing(target / name, changed)
            result = subprocess.run([str(SDK), 'finalize-auction', str(target)], cwd=ROOT, env=ENV,
                capture_output=True, timeout=120)
            write_bytes(target / (label + '.log'), result.stdout + result.stderr)
            if result.returncode == 0 or diagnostic.encode() not in result.stderr:
                raise RuntimeError('Expected memo pre-sign refusal absent: ' + label)
            if (target / 'synthetic-nu7-transaction.bin').exists():
                raise RuntimeError('Rejected memo checkpoint emitted transaction')
            rejected.append(label)
        finally:
            if name: write_existing(target / name, data)
    if any(hashlib.sha256(private_read(source / n)).hexdigest() != digest for n,digest in originals.items()):
        raise RuntimeError('Original checkpoint changed')
    summary = {'scope':'unsigned six-output native memo preparation; pre-sign rejection only',
        'rejected':rejected,'source_unchanged':True,'native_application_proof_generated':False,
        'threshold_signature_created':False,'transaction_emitted':False,'broadcast':False,
        'seconds':time.monotonic()-start}
    private_write(target / 'summary.json', summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
