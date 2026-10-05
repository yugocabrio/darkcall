#!/usr/bin/env python3
"""Own-ticket memo opening from the same certified funded NU7 public bundle.

Both fixed C++ proofs, public receipt linkage and admission authentication are
checked BEFORE reading the exported own viewing identity. Full native row proof
verification and chain inclusion remain separate explicit checks.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from check_ticket_receipts import public_book
from funded_public_bundle import snapshot
from public_bundle import verify_flags
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_nu7_funding import SDK
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

INPUT = ROOT / 'target/release/darkcall-input-tool'


def open_receipt(expected_book, bundle, authority, own_identity, index, name):
    if (not name or len(name) > 50 or not 0 <= index < 4
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name)):
        raise RuntimeError('Own ticket index and fresh ignored recipient name required')
    target = ROOT / '.local' / name
    target.mkdir(mode=0o700)
    started = time.monotonic()
    write_bytes(target / 'expected-book.json', public_book(pathlib.Path(expected_book)))
    frozen = snapshot(bundle,authority,'own-funded-public')
    flags = frozen / 'receipt-proof-public'
    verify_flags(flags,'own-funded-flags')
    with (target / 'fixed-auction-proof.log').open('xb') as log:
        checked = subprocess.run(['python3',str(ROOT / 'scripts/verify_external.py'),
            str(frozen / 'mpc-public'),'--circuit','settlement_poseidon4','--output-label','own-funded-auction'],
            cwd=ROOT,env=ENV,stdout=log,stderr=subprocess.STDOUT,timeout=120)
    if checked.returncode: raise RuntimeError('Fixed funded auction proof rejected before viewing-key access')
    certified = subprocess.run([str(SDK),'funded-certificate-link',str(frozen),
        str(frozen / 'trusted-authority.json')],cwd=ROOT,env=ENV,capture_output=True,timeout=30)
    if certified.returncode or json.loads(certified.stdout)['accepted_book_and_recipients_authenticated'] is not True:
        raise RuntimeError('Funded admission certificate rejected before viewing-key access')
    # This is the ONLY private identity read by this command. No wallet's
    # spending key, other actor's identity, escrow package or PCZT is accessed.
    write_bytes(target / f'private-funded-recipient-{index}.json', private_read(pathlib.Path(own_identity),4096))
    write_bytes(target / 'funded-nu7-transaction.bin', private_read(frozen / 'funded-nu7-transaction.bin',100000))
    memo = target / 'private-received.memo'
    native = guarded_json_call([str(SDK),'funded-receipt-open',str(target),str(index),str(memo)],
        cwd=ROOT,env=ENV,timeout=120,observations=target / 'native-resources.jsonl')
    with (target / 'private-opening-error.log').open('xb') as log:
        opened = subprocess.run([str(INPUT),'flags-ticket-memo-open',str(target / 'expected-book.json'),
            str(index),str(memo),str(flags / 'public-input0')],cwd=ROOT,env=ENV,
            stdout=subprocess.PIPE,stderr=log,timeout=30)
    if opened.returncode or len(opened.stdout) > 4096:
        raise RuntimeError('Funded memo rejected against expected registration book/ticket')
    value = json.loads(opened.stdout)
    if (not value['opening_verified'] or value['proof_verified_here']
            or not native['native_proof_and_signatures_verified'] or not native['own_viewing_key_only']
            or native['amount'] != value['refund']):
        raise RuntimeError('Funded native refund differs from authenticated ticket opening')
    result = {'scope':'own-ticket certified NU7 memo receipt; chain inclusion checked separately',
        'ticket_index':index,'txid':native['txid'],'won':value['won'],'refund':value['refund'],
        'price':value['price'],'sold':value['sold'],'fixed_cpp_ticket_and_auction_proofs_verified':True,
        'accepted_book_and_recipients_authenticated':True,'own_ticket_opening_verified':True,
        'native_amount_matches_refund':True,'native_proof_and_signatures_verified':True,
        'own_viewing_key_only':True,'other_ticket_pads_or_spending_keys_read':False,
        'receipt_in_same_six_output_transaction':True,'six_native_row_proofs_verified_here':False,
        'chain_inclusion_verified_here':False,'seconds':time.monotonic() - started}
    private_write(target / 'private-receipt-result.json',result)
    return result


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('expected_book','public_bundle','trusted_authority','own_viewing_identity'):
        parser.add_argument(name,type=pathlib.Path)
    parser.add_argument('index',type=int)
    parser.add_argument('--name',required=True)
    args = parser.parse_args()
    print(json.dumps(open_receipt(args.expected_book,args.public_bundle,args.trusted_authority,
        args.own_viewing_identity,args.index,args.name),indent=2))


if __name__ == '__main__': main()
