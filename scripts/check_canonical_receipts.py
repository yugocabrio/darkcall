#!/usr/bin/env python3
"""Open all four actual memos from ONE completed six-output synthetic auction.

Reject proof/cross-run/native/own-identity substitutions. Public verification
and chain inclusion are separate measurements; no spending keys are read here.
"""
import argparse
import json
import os
import pathlib
import secrets
import subprocess
import time
from open_ticket_memo import open_receipt
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run',type=pathlib.Path)
    parser.add_argument('bundle',type=pathlib.Path)
    args=parser.parse_args()
    run=args.run.resolve(strict=True)
    bundle=args.bundle.resolve(strict=True)
    proof=run/'receipt-proof-public'
    started=time.monotonic()
    nonce=secrets.token_hex(5)
    outcomes=[]
    for i in range(4):
        name=f'canonical-ticket-{nonce}-{i}'
        outcome=open_receipt(run/'receipt-book.json',proof,run,i,name,auction_bundle=bundle)
        if not outcome['receipt_in_six_output_transaction'] or not outcome['fixed_auction_context_proof_verified']:
            raise RuntimeError('Receipt was not linked to the same six-output auction')
        target=ROOT/'.local'/name
        own=[p.name for p in target.glob('private-*recipient*.json')]
        if own != [f'private-auction-recipient-{i}.json']:
            raise RuntimeError('Recipient copied other identities')
        outcomes.append(outcome)
    if len({v['txid'] for v in outcomes}) != 1 or sum(v['won'] for v in outcomes) != outcomes[0]['sold']:
        raise RuntimeError('Four receipt results do not form one auction')
    rejected=[]
    for attack in ('proof','wrong-key','trailing-tx','cross-run-context'):
        source=ROOT/'.local'/f'canonical-attack-{nonce}-{attack}'
        source.mkdir(mode=0o700)
        receipt_proof=source/'proof'
        receipt_proof.mkdir(mode=0o700)
        for name in ('vk','proof.0','public-input0'):
            data=private_read(proof/name)
            if attack=='proof' and name=='proof.0': data=bytes([data[0]^1])+data[1:]
            write_bytes(receipt_proof/name,data)
        identity=private_read(run/'private-auction-recipient-0.json')
        if attack=='wrong-key':
            value=json.loads(identity)
            value['viewing_key']=json.loads(private_read(run/'private-auction-recipient-1.json'))['viewing_key']
            identity=json.dumps(value).encode()
        write_bytes(source/'private-auction-recipient-0.json',identity)
        tx=private_read(run/'synthetic-nu7-transaction.bin')
        write_bytes(source/'synthetic-nu7-transaction.bin',tx+b'\0' if attack=='trailing-tx' else tx)
        supplied_bundle=bundle
        if attack=='cross-run-context':
            # A genuine separately proved zero-price flags statement is not a
            # replacement for this positive auction's receipt context.
            historical=ROOT/'.local/zero-receipt-memo-v2-20261005/proof-public'
            for name in ('vk','proof.0','public-input0'):
                (receipt_proof/name).unlink()
                write_bytes(receipt_proof/name,private_read(historical/name))
        name=f'canonical-reject-{nonce}-{attack}'
        checked=subprocess.run(['python3',str(ROOT/'scripts/open_ticket_memo.py'),str(run/'receipt-book.json'),
            str(receipt_proof),str(source),'0','--name',name,'--auction-bundle',str(supplied_bundle)],
            cwd=ROOT,env=ENV,capture_output=True,timeout=120)
        expected={'proof':'Fixed C++ ticket proof rejected before native decryption',
            'wrong-key':'recipient cannot decrypt private receipt',
            'trailing-tx':'Recipient transaction differs from canonical auction bundle',
            'cross-run-context':'Receipt and settlement auction context/price/sold differ'}[attack]
        if checked.returncode==0 or expected.encode() not in checked.stderr:
            raise RuntimeError('Intended canonical receipt rejection absent: '+attack)
        write_bytes(source/'rejection.log',checked.stderr)
        target=ROOT/'.local'/name
        if attack in ('proof','cross-run-context') and (target/'private-auction-recipient-0.json').exists():
            raise RuntimeError('Invalid proof/context read the recipient key')
        rejected.append(attack)
    summary={'scope':'same synthetic NU7 six-output memo auction; not live funding',
        'four_receipts':outcomes,'rejected':rejected,'other_identities_copied':False,
        'seconds':time.monotonic()-started}
    private_write(ROOT/'.local'/f'canonical-receipts-{nonce}-summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__=='__main__': main()
