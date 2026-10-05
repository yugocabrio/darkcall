#!/usr/bin/env python3
"""Exercise the actual recipient command against frozen synthetic transport runs."""
import argparse
import json
import os
import pathlib
import time
from open_ticket_memo import open_receipt
from regtest_deposits import ROOT, private_read, private_write
from regtest_prepared_gate import write_bytes


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('transport',type=pathlib.Path)
    parser.add_argument('--name',required=True)
    args=parser.parse_args()
    local=(ROOT/'.local').resolve()
    transport=args.transport.resolve(strict=True)
    if (not transport.is_relative_to(local) or not args.name or len(args.name)>30
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Expected synthetic transport and fresh short client name')
    target=local/args.name
    target.mkdir(mode=0o700)
    proof=transport/'proof-public'
    expected=json.loads(private_read(transport/'summary.json',32768))
    results=[]
    started=time.monotonic()
    for index in range(4):
        opened=open_receipt(proof/'book.json',proof,transport/f'recipient-{index}',index,args.name+f'-ticket-{index}')
        if opened['won']!=expected['recipients'][index]['won'] or opened['refund']!=expected['recipients'][index]['refund']:
            raise RuntimeError('Recipient command differs from proved expected result')
        results.append(opened)
    rejects=[]
    for attack_name in ('uploaded-vk','invalid-proof','changed-statement','truncated-statement','wrong-book','wrong-ticket'):
        source=target/attack_name
        source.mkdir(mode=0o700)
        pub=source/'public'
        pub.mkdir(mode=0o700)
        for name in ('vk','proof.0','public-input0'):
            value=bytearray(private_read(proof/name,1048576))
            if name=='vk' and attack_name=='uploaded-vk': value[0]^=1
            if name=='proof.0' and attack_name=='invalid-proof': value[-1]^=1
            if name=='public-input0' and attack_name=='changed-statement': value[183*32+31]^=1
            if name=='public-input0' and attack_name=='truncated-statement': value=value[:-1]
            write_bytes(pub/name,value)
        book=json.loads(private_read(proof/'book.json',32768))
        if attack_name=='wrong-book': book['condition_digest'][0]^=1
        private_write(source/'book.json',book)
        index=1 if attack_name=='wrong-ticket' else 0
        for name in ('synthetic-receipt-transport.bin','private-recipient-0.json'):
            name_out=f'private-recipient-{index}.json' if name.startswith('private-recipient') else name
            write_bytes(source/name_out,private_read(transport/'recipient-0'/name,100000))
        checkpoint_name=args.name+'-'+attack_name
        expected_error={'truncated-statement':'exact blinded v2',
            'wrong-book':'recipient expected book/ticket','wrong-ticket':'recipient expected book/ticket'}.get(attack_name,'Fixed C++')
        try:
            open_receipt(source/'book.json',pub,source,index,checkpoint_name)
        except RuntimeError as error:
            if expected_error not in str(error): raise RuntimeError('Unexpected recipient error path') from error
        else: raise RuntimeError('Tampered recipient command accepted')
        checkpoint=local/checkpoint_name
        if (checkpoint/'private-receipt-result.json').exists():
            raise RuntimeError('Rejected command emitted a verified receipt result')
        if attack_name not in ('wrong-book','wrong-ticket') and (
                (checkpoint/'private-received.memo').exists() or
                list(checkpoint.glob('private-recipient-*.json'))):
            raise RuntimeError('Invalid proof consumed private receiver data before verification')
        rejects.append(attack_name)
    summary={'scope':'actual single-command recipient client; local synthetic memo transport',
        'passed':True,'results':results,'client_attack_rejections':rejects,
        'invalid_proof_does_not_read_copy_receiver_key_or_decrypt_memo':True,
        'public_negative_fixtures_required_from_producer':False,
        'seconds':time.monotonic()-started,'chain_inclusion_verified':False,
        'six_output_auction_settlement_verified':False}
    private_write(target/'summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
