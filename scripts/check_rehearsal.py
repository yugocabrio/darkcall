#!/usr/bin/env python3
"""Actual local fresh input proofs/receipts; HTTP and pre-admission substitutions.

Never native funding/signing, RPC, MPC clearing or recorded fixture mutation.
"""
import argparse
import hashlib
import json
import pathlib
import secrets
import subprocess
import time
import urllib.request
import urllib.error
from regtest_deposits import ROOT, private_read, private_write
from rehearsal import HELPER


def run(port):
    origin='http://127.0.0.1:'+str(port); started=time.monotonic()
    def call(path,value=None,extra=None):
        headers={'Origin':origin}
        if value is not None: headers.update({'Content-Type':'application/json','X-DarkCall-Token':state['csrf']})
        if extra: headers.update(extra)
        req=urllib.request.Request(origin+path,data=None if value is None else json.dumps(value).encode(),headers=headers)
        try:
            with urllib.request.urlopen(req,timeout=180) as response: return response.status,json.load(response)
        except urllib.error.HTTPError as e: return e.code,json.load(e)
    _,state=call('/api/state')
    assert state['local_rehearsal_available']
    expected_tx=state['cases'][0]['txid']
    denied=[]
    base={'units':2,'reserve':20,'budget':100,'closes_at_utc':int(time.time())+75}
    for label,value,headers,status in [
        ('zero-units',{**base,'units':0},{},400),('reserve-over-budget',{**base,'reserve':101},{},400),
        ('negative-budget',{**base,'budget':-1},{},400),('float-unit',{**base,'units':2.0},{},400),
        ('past-cutoff',{**base,'closes_at_utc':1},{},400),('extra-authority',{**base,'authority':'supplied'},{},400),
        ('cross-origin',base,{'Origin':'https://attacker.invalid'},403),
        ('wrong-token',base,{'X-DarkCall-Token':'bad'},403),
    ]:
        assert call('/api/rehearsal/create',value,headers)[0]==status,label
        denied.append(label)
    status,auction=call('/api/rehearsal/create',base); assert status==200,auction
    identifier=auction['auction_id']; path=next((ROOT/'.local').glob('browser-rehearsal-*/'+identifier))
    receipt_results=[]
    for value in [90,75,50,0]:
        status,result=call('/api/rehearsal/bid',{'auction_id':identifier,'maximum_bid':value}); assert status==200,result
        assert result['accepted'] and result['client_input_proof_verified'] and result['receipt_signatures_verified']==3 and result['share_pairs_verified']==3
        assert not result['chain_funding_verified'] and not result['native_settlement_started']
        assert len(result['receipt'])==3
        assert not {'maximum_bid','bid','salt','bid_shares','salt_shares','viewing_key','private_path'} & result.keys()
        receipt_results.append(result)
    for label,value in [('over-budget',101),('negative-bid',-1),('bool-bid',True),('fifth-ticket',5)]:
        assert call('/api/rehearsal/bid',{'auction_id':identifier,'maximum_bid':value})[0]==400,label
        denied.append(label)
    assert call('/api/rehearsal/close',{'auction_id':identifier})[0]==400
    denied.append('early-final-book')
    journals=[path/f'private-node-{p}.jsonl' for p in range(3)]
    hashes=lambda:[hashlib.sha256(private_read(p)).hexdigest() for p in journals]
    before=hashes(); client=next(path.glob('client-*'))
    book=json.loads(private_read(client/'book.json')); i=book['active'].index(True)
    # Fresh attack directories: original logs/journals/private source are untouched.
    for label in ['proof','pair','context']:
        folder=path/('attack-'+label+'-'+secrets.token_hex(4)); folder.mkdir(mode=0o700)
        for name in ['book.json','proof_plaindriver','public_inputs_plaindriver']+[f'bidder{i}.toml.{p}.shared' for p in range(3)]:
            content=private_read(client/name)
            if label=='proof' and name=='proof_plaindriver': content=bytes([content[0]^1])+content[1:]
            if label=='pair' and name==f'bidder{i}.toml.0.shared':
                # Alter an encoded share while preserving file size.
                content=content[:-1]+bytes([content[-1]^1])
            if label=='context' and name=='book.json':
                wrong=json.loads(content); wrong['condition_digest'][0]^=1; content=json.dumps(wrong).encode()
            fd=folder/name; fd.write_bytes(content); fd.chmod(0o600)
        with (folder/'attempt.log').open('wb') as log:
            rejected=subprocess.run([str(HELPER),'admit',str(path),str(folder),str(i)],stdout=log,stderr=log,timeout=180)
        assert rejected.returncode!=0,label
        assert before==hashes(),label+' mutated admission journals'
        denied.append('actual-'+label+'-substitution')
    while int(time.time())<auction['closes_at_utc']: time.sleep(.2)
    status,closed=call('/api/rehearsal/close',{'auction_id':identifier}); assert status==200,closed
    assert closed['closed'] and len(closed['certificate']['endorsements'])==3 and not closed['fresh_clearing_or_settlement']
    assert call('/api/rehearsal/bid',{'auction_id':identifier,'maximum_bid':50})[0]==400
    denied.append('late-bid')
    assert call('/api/rehearsal/close',{'auction_id':identifier})[1]==closed
    assert call('/api/state')[1]['cases'][0]['txid']==expected_tx
    return {'passed':True,'scope':'fresh offline browser rehearsal; single-client input ZK + REP3 pairs + signed admission, not MPC clearing',
        'client_proofs':4,'verified_share_pairs':12,'acceptance_signatures':12,'final_book_signatures':3,
        'actual_pre_admission_substitutions_leave_journals_unchanged':True,'rejected':denied,
        'per_bid_seconds':[round(r['seconds'],3) for r in receipt_results],
        'new_auction_condition_digest':bytes(auction['condition_digest']).hex(),'new_accepted_root':closed['accepted_root'],
        'recorded_txid_unchanged':expected_tx,'secret_fields_served':False,'synthetic_funding_observations':True,
        'actual_chain_funding':False,'fresh_mpc_clearing_or_settlement':False,'clock_cutoff_enforced':True,
        'seconds':time.monotonic()-started}


if __name__=='__main__':
    import os; os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--port',type=int,default=8877); parser.add_argument('--output',type=pathlib.Path,required=True)
    args=parser.parse_args(); result=run(args.port)
    private_write(args.output,result); print(json.dumps(result,indent=2))
