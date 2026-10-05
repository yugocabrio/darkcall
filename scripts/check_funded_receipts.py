#!/usr/bin/env python3
"""Four own-ticket openings and substitutions for one funded NU7 transaction.

Only exported own viewing identities are used, not wallet spending keys. Mutated
public copies have recomputed packaging hashes, so fixed verifiers must refuse.
Native row proofs and controlled chain confirmations are separate measurements.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets
import subprocess
import time

from funded_public_bundle import export, snapshot
from open_funded_ticket import open_receipt
from public_bundle import read_public
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from replay_nu7_auction import write_existing


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('run', 'bundle', 'authority'):
        parser.add_argument(key, type=pathlib.Path)
    args = parser.parse_args()
    run = args.run.resolve(strict=True)
    bundle = args.bundle.resolve(strict=True)
    authority = args.authority.resolve(strict=True)
    source_hashes = {str(p): hashlib.sha256(read_public(p.parent, p.name, 32768)).hexdigest()
                     for p in (bundle / 'bundle.json', authority)}
    nonce = secrets.token_hex(5)
    started = time.monotonic()
    results = []
    for i in range(4):
        name = f'funded-own-{nonce}-{i}'
        result = open_receipt(run / 'receipt-book.json', bundle, authority,
                              run / f'private-funded-recipient-{i}.json', i, name)
        identities = [p.name for p in (ROOT / '.local' / name).glob('private-*recipient*.json')]
        if identities != [f'private-funded-recipient-{i}.json']:
            raise RuntimeError('Own funded client copied another viewing identity')
        if (result['refund'] != (50 if i < 3 else 100) or result['won'] != (i < 3)
                or result['price'] != 50 or result['sold'] != 3
                or result['six_native_row_proofs_verified_here'] or result['chain_inclusion_verified_here']):
            raise RuntimeError('Funded canonical receipt result differs from measured scope')
        results.append(result)
    if len({v['txid'] for v in results}) != 1:
        raise RuntimeError('Own clients opened different transactions')

    expected = {
        'flags-proof': 'Fixed C++ ticket proof rejected before native decryption',
        'auction-proof': 'Fixed funded auction proof rejected before viewing-key access',
        'certificate': 'Funded admission certificate rejected before viewing-key access',
        'wrong-key': 'recipient cannot decrypt private receipt',
        'trailing-tx': 'trailing data',
        'wrong-book': 'Funded memo rejected against expected registration book/ticket',
    }
    rejected = []
    for attack, diagnostic in expected.items():
        frozen = snapshot(bundle, authority, 'funded-client-attack')
        supplied_authority = frozen / 'trusted-authority.json'
        target = ROOT / '.local' / f'funded-copy-{nonce}-{attack}'
        target.mkdir(mode=0o700)
        own = private_read(run / 'private-funded-recipient-0.json')
        if attack == 'wrong-key':
            value = json.loads(own)
            value['viewing_key'] = json.loads(private_read(run / 'private-funded-recipient-1.json'))['viewing_key']
            own = json.dumps(value).encode()
        write_bytes(target / 'own.json', own)
        book = private_read(run / 'receipt-book.json')
        if attack == 'wrong-book':
            value = json.loads(book)
            value['ids'][0] = value['ids'][1]
            book = json.dumps(value).encode()
        write_bytes(target / 'book.json', book)
        path = {'flags-proof': 'receipt-proof-public/proof.0',
                'auction-proof': 'mpc-public/proof.0',
                'certificate': 'final-book-certificate.json',
                'trailing-tx': 'funded-nu7-transaction.bin'}.get(attack)
        if path:
            data = private_read(frozen / path)
            if attack == 'certificate':
                value = json.loads(data)
                sig = bytearray.fromhex(value['endorsements'][0]['signature'])
                sig[0] ^= 1
                value['endorsements'][0]['signature'] = sig.hex()
                changed = json.dumps(value).encode()
            else:
                changed = data + b'\0' if attack == 'trailing-tx' else bytes([data[0] ^ 1]) + data[1:]
            write_existing(frozen / path, changed)
        forged = target / 'public'
        export(frozen, forged)
        name = f'funded-reject-{nonce}-{attack}'
        checked = subprocess.run(['python3', str(ROOT / 'scripts/open_funded_ticket.py'),
            str(target / 'book.json'), str(forged), str(supplied_authority), str(target / 'own.json'),
            '0', '--name', name], cwd=ROOT, env=ENV, capture_output=True, timeout=150)
        write_bytes(target / 'rejection.log', checked.stderr)
        if checked.returncode == 0 or diagnostic.encode() not in checked.stderr or b'panicked at' in checked.stderr:
            raise RuntimeError('Intended funded client refusal absent: ' + attack)
        opened = ROOT / '.local' / name
        if attack in ('flags-proof', 'auction-proof', 'certificate') and (
                (opened / 'private-funded-recipient-0.json').exists()
                or (opened / 'private-received.memo').exists()):
            raise RuntimeError('Invalid public proof/certificate accessed recipient identity')
        rejected.append(attack)
    for path, digest in source_hashes.items():
        p = pathlib.Path(path)
        if hashlib.sha256(read_public(p.parent, p.name, 32768)).hexdigest() != digest:
            raise RuntimeError('Original public bundle or authority changed')
    summary = {'scope': 'same funded NU7 transaction: four own clients plus copied substitutions',
        'four_receipts': results, 'rejected': rejected,
        'invalid_public_proofs_or_certificate_before_key': True,
        'other_identities_copied': False, 'source_unchanged': True,
        'seconds': time.monotonic() - started}
    private_write(ROOT / '.local' / f'funded-receipts-{nonce}-summary.json', summary)
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
