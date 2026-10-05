#!/usr/bin/env python3
"""Actual encrypted NU7 funding claims: read-only replay and substitution checks.

Makes fresh ignored copies. Never issues receipts, writes source journals or
signs/broadcasts a transaction. Funding provenance is the node driver's task.
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

FILES = ('private-book.json', 'private-deposit-map.json',
         'private-claims.json', 'nu7-regtest-transaction.bin')


def mutate(label, data):
    if label == 'trailing-transaction':
        data[FILES[3]] += b'\0'
        return
    if label == 'old-branch':
        tx = bytearray(data[FILES[3]])
        if tx[8:12] != bytes.fromhex('d90a1977'):
            raise RuntimeError('Unexpected actual v6 NU7 encoding')
        tx[8:12] = bytes.fromhex('5b16a537')
        data[FILES[3]] = bytes(tx)
        return
    name = FILES[2] if label.startswith('claim-') else FILES[1] if label.startswith('map-') else FILES[0]
    value = json.loads(data[name])
    if label == 'claim-signature': value[0]['signature'][0] ^= 1
    elif label == 'claim-note': value[0]['note_id'][0] ^= 1
    elif label == 'claim-memo': value[0]['memo_digest'][0] ^= 1
    elif label == 'claim-bid': value[0]['instructions']['bid_commitment'][31] ^= 1
    elif label == 'claim-pair': value[0]['instructions']['pair_commitments'][0][31] ^= 1
    elif label == 'claim-refund': value[0]['instructions']['refund_receiver_commitment'][31] ^= 1
    elif label == 'map-action': value['indices'][0] = 100
    elif label == 'map-refund': value['receiver_commitments'][0][31] ^= 1
    elif label == 'book-context': value['condition_digest'][0] ^= 1
    elif label == 'book-ticket': value['ids'][0][0] ^= 1
    elif label == 'book-budget': value['budget'] += 1
    else: raise RuntimeError('Unknown bounded claim check')
    data[name] = json.dumps(value).encode()


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('escrow', type=pathlib.Path)
    parser.add_argument('deposit', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 60
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh simple ignored check name required')
    out = ROOT/'.local'/args.name
    out.mkdir(mode=0o700)
    start = time.monotonic()
    original = {name:private_read(args.deposit/name) for name in FILES}
    hashes = {name:hashlib.sha256(value).hexdigest() for name,value in original.items()}
    result = subprocess.run([str(SDK),'regtest-claim-check',str(args.escrow),str(args.deposit)],
        cwd=ROOT,env=ENV,capture_output=True,timeout=60)
    if result.returncode:
        raise RuntimeError('Actual NU7 claim replay failed')
    verified = json.loads(result.stdout)
    labels = ('claim-signature','claim-note','claim-memo','claim-bid','claim-pair',
        'claim-refund','map-action','map-refund','book-context','book-ticket','book-budget',
        'trailing-transaction','old-branch')
    rejected = []
    for label in labels:
        case = out/label
        case.mkdir(mode=0o700)
        values = original.copy()
        mutate(label,values)
        for name,value in values.items():
            fd = os.open(case/name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
            with os.fdopen(fd,'wb') as file: file.write(value)
        result = subprocess.run([str(SDK),'regtest-claim-check',str(args.escrow),str(case)],
            cwd=ROOT,env=ENV,capture_output=True,timeout=60)
        if result.returncode == 0 or b'panicked at' in result.stderr:
            raise RuntimeError('Actual NU7 claim substitution not safely rejected: '+label)
        rejected.append(label)
    if any(hashlib.sha256(private_read(args.deposit/name)).hexdigest() != hashes[name] for name in FILES):
        raise RuntimeError('Actual source changed during claim checks')
    summary = {'scope':'actual NU7 ciphertext/intent replay; no admission or chain proof',
        'verified':verified,'rejected':rejected,'source_unchanged':True,
        'receipts_or_journals_written':False,'seconds':time.monotonic()-start}
    private_write(out/'summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__ == '__main__': main()
