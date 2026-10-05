#!/usr/bin/env python3
"""Actual native NU7 threshold transfer on the owned loopback regtest only.

Ordinary bank or existing local DKG source; no auction-policy claim. Never
reuses old NU6.3 state or sends a synthetic/public-network transaction.
"""
import argparse
import json
import os
import pathlib
import subprocess
from regtest_nu7_node import ROOT, GENESIS, BRANCH, rpc, status
from regtest_nu7_funding import SDK, capture
from regtest_funding import tree
from regtest_deposits import ENV, private_read, private_write


def verify_source(notes, snapshot):
    if (notes['scope'] != 'isolated NU7 regtest wallet openings'
            or notes['genesis'] != GENESIS or notes['branch'] != BRANCH
            or notes['nu7_height'] != 2 or notes['tip_height'] != snapshot['blocks']
            or notes['tip_hash'] != snapshot['tip']):
        raise RuntimeError('NU7 source is not the current isolated snapshot')
    node = rpc('z_gettreestate', [snapshot['tip']])
    if bytes(notes['anchor']).hex() != node['ironwood']['commitments']['finalRoot']:
        raise RuntimeError('NU7 source anchor differs from canonical full node')
    for note in notes['notes']:
        block_hash = bytes(note['mined_block'])[::-1].hex()
        if rpc('getblockhash', [note['mined_height']]) != block_hash:
            raise RuntimeError('NU7 source funding block is no longer canonical')
        block = rpc('getblock', [block_hash,2])
        txid = bytes(note['txid'])[::-1].hex()
        tx = next(t for t in block['tx'] if t['txid'] == txid)
        if tx['ironwood']['actions'][note['action_index']]['cmx'] != bytes(note['cmx']).hex():
            raise RuntimeError('NU7 source cmx differs from actual node transaction')
        coinbase = bool(tx['vin']) and 'coinbase' in tx['vin'][0]
        if coinbase != note['coinbase'] or tx['confirmations'] < (101 if coinbase else 2):
            raise RuntimeError('NU7 source maturity/confirmation provenance differs')


def verify_reconciliation(notes, constructed, recipient_scan, source_scan, remaining, provenance):
    expected_balance = sum(n['amount'] for n in notes['notes'])-constructed['recipient_amount']-constructed['planned_fee']
    if (recipient_scan['decrypted_unspent_native_notes'] != 1
            or recipient_scan['balance_zatoshi'] != constructed['recipient_amount']
            or source_scan['spent_native_notes'] < 1
            or source_scan['balance_zatoshi'] != expected_balance
            or any(n['cmx'] == provenance['source_cmx'] for n in remaining['notes'])):
        raise RuntimeError('NU7 exact recipient amount/source consumption was not observed')


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('wallet', type=pathlib.Path)
    parser.add_argument('notes', type=pathlib.Path)
    parser.add_argument('recipient', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)
            or (ROOT/'.local'/args.name).exists()):
        raise RuntimeError('Fresh ignored NU7 transfer name required')
    if args.wallet.resolve() == args.recipient.resolve():
        raise RuntimeError('This measured transfer gate requires a separate recipient wallet')
    snapshot = status()
    notes = json.loads(private_read(args.notes))
    verify_source(notes,snapshot)
    receiver_birthday = args.recipient/'nu7-birthday.json'
    if receiver_birthday.exists():
        initial = json.loads(private_read(receiver_birthday))
    else:
        initial = tree(rpc('z_gettreestate', [snapshot['tip']]))
        private_write(receiver_birthday,initial)
    # The source wallet's retained birth snapshot is required for nullifier
    # reconciliation; do not silently invent a new source birthday.
    birthday = json.loads(private_read(args.wallet/'nu7-birthday.json'))
    bad = json.loads(json.dumps(notes))
    index = next(i for i,n in enumerate(bad['notes'])
                 if not n['coinbase'] or snapshot['blocks'] >= n['mined_height']+100)
    bad['notes'][index]['auth_path'][0][0] ^= 1
    wrong = args.wallet/(args.name+'-wrong-witness.json')
    private_write(wrong,bad)
    rejected = subprocess.run([str(SDK),'regtest-transfer',str(args.wallet),str(wrong),
        str(args.recipient),args.name+'-reject'],cwd=ROOT,env=ENV,capture_output=True,timeout=60)
    if rejected.returncode == 0 or b'NU7 source witness differs from node anchor' not in rejected.stderr:
        raise RuntimeError('Altered actual NU7 source path was not refused before proving')
    constructed = json.loads(subprocess.check_output([str(SDK),'regtest-transfer',str(args.wallet),
        str(args.notes),str(args.recipient),args.name],cwd=ROOT,env=ENV,text=True,timeout=600))
    output = ROOT/'.local'/args.name
    if status()['tip'] != snapshot['tip']:
        raise RuntimeError('NU7 chain changed during transfer construction; reconcile before broadcast')
    verify_source(notes,snapshot)
    binary = private_read(output/'nu7-regtest-transaction.bin')
    accepted = rpc('sendrawtransaction',[binary.hex()],timeout=180)
    if accepted != constructed['txid']:
        raise RuntimeError('NU7 node accepted another transfer identity')
    rpc('generate',[2],timeout=300)
    tx = rpc('getrawtransaction',[accepted,1])
    if not tx['in_active_chain'] or tx['confirmations'] < 2:
        raise RuntimeError('NU7 transfer is not canonical with two controlled confirmations')
    final = status()
    recipient_scan, _ = capture(args.recipient,initial,final)
    source_scan, remaining_path = capture(args.wallet,birthday,final)
    remaining = json.loads(private_read(remaining_path))
    provenance = json.loads(private_read(output/'private-provenance.json'))
    verify_reconciliation(notes,constructed,recipient_scan,source_scan,remaining,provenance)
    if status()['tip'] != final['tip']:
        raise RuntimeError('NU7 chain changed during wallet reconciliation')
    result = {'scope':'actual isolated NU7 threshold transfer, not auction settlement',
        'construction':constructed,'full_node_accepted':True,'canonical_confirmations':tx['confirmations'],
        'recipient_scan':recipient_scan,'source_scan':source_scan,'altered_witness_rejected':True,
        'old_regtest_changed':False,'public_testnet':False,'mpc_auction_or_admission':False,
        'finality_scope':'two controlled local confirmations; not public-chain finality'}
    private_write(output/'confirmed-summary.json',result)
    print(json.dumps(result,indent=2))


if __name__ == '__main__': main()
