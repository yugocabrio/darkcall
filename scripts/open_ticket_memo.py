#!/usr/bin/env python3
"""Recipient command: fixed proof verification, native authentication and memo opening.

Experimental ignored synthetic fixtures only. The caller supplies their own
expected book/ticket and native viewing identity; those are not learned from
the receipt. The transport transfer is separate from auction settlement.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from check_ticket_receipts import public_book
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'
INPUT = ROOT / 'target/release/darkcall-input-tool'


def open_receipt(book, proof, receiver, index, name):
    """Freeze inputs into a new private recipient checkpoint; no secret logs returned."""
    local = (ROOT / '.local').resolve()
    if (not name or len(name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name)
            or not 0 <= index < 4):
        raise RuntimeError('Expected recipient ticket index and unused simple local name')
    book, proof, receiver = map(pathlib.Path, (book, proof, receiver))
    if any(not p.resolve(strict=True).is_relative_to(local) for p in (book,proof,receiver)):
        raise RuntimeError('Recipient command accepts ignored synthetic fixtures only')
    target = local / name
    target.mkdir(mode=0o700)
    started = time.monotonic()
    public = target / 'proof-public'
    public.mkdir(mode=0o700)
    write_bytes(target / 'expected-book.json', public_book(book))
    statement = private_read(proof / 'public-input0',187*32)
    if len(statement) != 187*32:
        raise RuntimeError('Recipient requires exact blinded v2 flags statement')
    write_bytes(public / 'public-input0',statement)
    for filename in ('vk','proof.0'):
        write_bytes(public / filename,private_read(proof / filename,1048576))
    # Generate negative statements from the frozen input. The client needs only
    # the real public proof/vk/statement, never producer-supplied attack fixtures.
    for kind,index_field in [('price',177),('root',168),('cipher',179),('winner',183)]:
        changed = bytearray(statement)
        value = int.from_bytes(changed[index_field*32:(index_field+1)*32],'big')
        changed[index_field*32:(index_field+1)*32] = (value-1 if value else 1).to_bytes(32,'big')
        write_bytes(public / ('tampered-'+kind),changed)
    with (target / 'fixed-cpp-verifier.log').open('xb') as log:
        verified = subprocess.run(['python3',str(ROOT/'scripts/verify_external.py'),str(public),
            '--circuit','delivery_flags4','--output-label','recipient-fixed-proof'],cwd=ROOT,env=ENV,
            stdout=log,stderr=subprocess.STDOUT,timeout=120)
    if verified.returncode:
        raise RuntimeError('Fixed C++ ticket proof rejected before native decryption')
    # Only after proof authentication do we read/copy this recipient's identity.
    # The native reader's strict schema/type/permissions checks remain mandatory.
    own_name = f'private-recipient-{index}.json'
    write_bytes(target / own_name,private_read(receiver/own_name,4096))
    write_bytes(target/'synthetic-receipt-transport.bin',private_read(receiver/'synthetic-receipt-transport.bin',100000))
    memo = target/'private-received.memo'
    native = guarded_json_call([str(SDK),'receipt-transport-open',str(target),str(index),str(memo)],
        cwd=ROOT,env=ENV,timeout=120,observations=target/'native-resources.jsonl')
    with (target/'ticket-opening-error.log').open('xb') as log:
        opened = subprocess.run([str(INPUT),'flags-ticket-memo-open',str(target/'expected-book.json'),
            str(index),str(memo),str(public/'public-input0')],cwd=ROOT,env=ENV,
            stdout=subprocess.PIPE,stderr=log,timeout=30)
    if opened.returncode or len(opened.stdout) > 4096:
        raise RuntimeError('Ticket memo rejected against recipient expected book/ticket')
    opening = json.loads(opened.stdout)
    if (not opening['opening_verified'] or opening['proof_verified_here']
            or not native['native_proof_and_signatures_verified']
            or not native['own_viewing_key_only'] or native['amount'] != opening['refund']):
        raise RuntimeError('Native received amount and ticket refund differ')
    result = {'scope':'recipient proof-backed encrypted memo; synthetic transport component',
        'ticket_index':index,'won':opening['won'],'refund':opening['refund'],
        'price':opening['price'],'sold':opening['sold'],'txid':native['txid'],
        'fixed_cpp_ticket_proof_verified':True,'native_proof_and_signatures_verified':True,
        'own_ticket_opening_verified':True,'native_amount_matches_refund':True,
        'own_viewing_key_only':True,'other_ticket_pads_read':False,
        'expected_book_and_ticket_source':'caller-provided registration expectation',
        'bidder_registration_authenticated_here':False,
        'six_output_auction_settlement_verified':False,'chain_inclusion_verified':False,
        'seconds':time.monotonic()-started}
    private_write(target/'private-receipt-result.json',result)
    return result


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('expected_book',type=pathlib.Path)
    parser.add_argument('public_proof',type=pathlib.Path)
    parser.add_argument('own_receiver',type=pathlib.Path)
    parser.add_argument('ticket_index',type=int)
    parser.add_argument('--name',required=True)
    args=parser.parse_args()
    print(json.dumps(open_receipt(args.expected_book,args.public_proof,args.own_receiver,args.ticket_index,args.name),indent=2))


if __name__ == '__main__':
    main()
