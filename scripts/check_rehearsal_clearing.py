#!/usr/bin/env python3
"""Fresh browser book → certified REP3 clearing → four own results, not payment.

Only synthetic fixture inputs. Never use this local helper for real bids.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
import urllib.error
import urllib.request
from regtest_deposits import ROOT, private_read, private_write
from rehearsal import HELPER


def run(port,zero=False):
    origin='http://127.0.0.1:'+str(port); started=time.monotonic()
    def call(path,value=None):
        headers={'Origin':origin}
        if value is not None: headers.update({'Content-Type':'application/json','X-DarkCall-Token':state['csrf']})
        req=urllib.request.Request(origin+path,data=None if value is None else json.dumps(value).encode(),headers=headers)
        try:
            with urllib.request.urlopen(req,timeout=600) as response: return response.status,json.load(response)
        except urllib.error.HTTPError as e: return e.code,json.load(e)
    _,state=call('/api/state');assert state['local_rehearsal_available']
    recorded_tx=state['cases'][0]['txid']
    bids=[0,0,0,0] if zero else [77,61,43,21]
    terms={'units':3 if zero else 2,'reserve':0 if zero else 10,'budget':100,'closes_at_utc':int(time.time())+40}
    code,auction=call('/api/rehearsal/create',terms);assert code==200,auction
    identifier=auction['auction_id'];scope={'auction_id':identifier}
    path=next((ROOT/'.local').glob('browser-rehearsal-*/'+identifier))
    assert call('/api/rehearsal/clear',scope)[0]==400
    accepted=[]
    for bid in bids:
        code,result=call('/api/rehearsal/bid',{**scope,'maximum_bid':bid});assert code==200,result
        assert result['receipt_signatures_verified']==3 and result['share_pairs_verified']==3
        assert result['opening_token']
        accepted.append(result)
    assert call('/api/rehearsal/clear',scope)[0]==400
    while int(time.time())<auction['closes_at_utc']:time.sleep(.2)
    code,closed=call('/api/rehearsal/close',scope);assert code==200,closed
    code,cleared=call('/api/rehearsal/clear',scope);assert code==200,cleared
    assert cleared['rep3_clearing_verified'] and cleared['fixed_cpp_proof_verified'] and cleared['certified_book_matches']
    assert cleared['accepted_root']==closed['accepted_root'] and cleared['certificate_signatures_verified']==3
    assert not cleared['native_settlement'] and not cleared['maximum_bid_openings_read']
    # Independent plaintext fixture oracle is ONLY a check of actual MPC output.
    order=sorted(range(4),key=lambda i:(-bids[i],accepted[i]['ticket']))
    qualified=[i for i in order if bids[i]>=terms['reserve']]
    winners=set(qualified[:terms['units']]);price=terms['reserve']
    if len(qualified)>terms['units']:price=bids[qualified[terms['units']]]
    assert cleared['price']==price and cleared['sold']==len(winners)
    assert call('/api/rehearsal/clear',scope)==(200,cleared) # No duplicate proof generation.
    public=call('/api/rehearsal/auction/'+identifier)[1]
    encoded=json.dumps(public)
    assert all(a['opening_token'] not in encoded for a in accepted)
    assert not any(k in encoded for k in ['opening_tokens','refund_pad','winner_pad','clearing_run'])
    request={**scope,'ticket':accepted[0]['ticket'],'opening_token':accepted[0]['opening_token'],'action':'open'}
    count=lambda:len(list(path.glob('result-*')))
    before=count()
    assert call('/api/rehearsal/result',{**request,'opening_token':'wrong'})[0]==403
    assert call('/api/rehearsal/result',{**request,'ticket':accepted[1]['ticket']})[0]==403
    assert count()==before # Refused before worker/pad read.
    results=[]
    for i,a in enumerate(accepted):
        code,result=call('/api/rehearsal/result',{**scope,'ticket':a['ticket'],'opening_token':a['opening_token'],'action':'open'})
        assert code==200,result
        assert result['won']==(i in winners) and result['refund']==100-(price if i in winners else 0)
        assert result['opening_verified'] and result['fixed_cpp_proof_verified'] and result['certified_book_matches']
        assert result['accepted_root']==closed['accepted_root'] and not result['other_ticket_pads_read'] and not result['maximum_bids_read']
        results.append({'won':result['won'],'refund':result['refund']})
    code,tampered=call('/api/rehearsal/result',{**request,'action':'tamper'});assert code==200,tampered
    assert tampered['rejected'] and not tampered['private_pad_read']
    # Prove statement/certificate linkage is checked independently of filenames.
    work=next(p for p in path.glob('result-*') if p.is_dir() and (p/'delivery-run.json').is_file())
    mpc=pathlib.Path(json.loads(private_read(work/'delivery-run.json'))['run'])
    refused=[]
    for field,name in [(0,'conditions'),(32,'ticket'),(164,'leaf'),(168,'root')]:
        target=path/('link-attack-'+name);target.mkdir(mode=0o700)
        data=bytearray(private_read(mpc/'public-input0'));data[field*32+31]^=1
        (target/'public-input0').write_bytes(data);(target/'public-input0').chmod(0o600)
        with (target/'attempt.log').open('xb') as log:
            r=subprocess.run([str(HELPER),'check-clearing',str(path),str(target)],stdout=log,stderr=log,timeout=30)
        assert r.returncode!=0,name;refused.append(name)
    # Actual modified certificate signature is rejected before pad opening.
    original=private_read(path/'final-certificate.json');certificate=json.loads(original)
    signature=bytearray.fromhex(certificate['endorsements'][0]['signature']);signature[0]^=1
    certificate['endorsements'][0]['signature']=signature.hex()
    (path/'final-certificate.json').write_bytes(json.dumps(certificate).encode())
    try:
        code,_=call('/api/rehearsal/result',request);assert code==422
        latest=max((p for p in path.glob('result-*') if p.is_dir()),key=lambda p:p.stat().st_mtime_ns)
        assert not (latest/'private-own-ticket.json').exists()
    finally:(path/'final-certificate.json').write_bytes(original)
    assert call('/api/state')[1]['cases'][0]['txid']==recorded_tx
    return {'passed':True,'scope':'fresh synthetic browser book; actual REP3 clearing; no native settlement',
        'price':price,'sold':len(winners),'own_results':results,'same_signed_root':closed['accepted_root'],
        'sorted_ticket_reindex':[next(j for j,e in enumerate(closed['certificate']['endorsements'][0]['book']['entries']) if bytes(e['ticket']).hex()==a['ticket']) for a in accepted],
        'client_proofs':4,'pair_checks':12,'acceptance_signatures':12,'final_book_signatures':3,
        'clearing':cleared,'wrong_ticket_capabilities_rejected_before_worker':2,
        'linkage_substitutions_rejected':refused,'bad_certificate_rejected_before_pad_read':True,
        'fixed_cpp_tamper_rejected_before_pad_read':True,'public_get_contains_no_capabilities':True,
        'idempotent_clear_reuses_verified_result':True,'recorded_txid_unchanged':recorded_tx,
        'zero_price':zero,'seconds':time.monotonic()-started}


if __name__=='__main__':
    os.umask(0o077)
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--port',type=int,default=8877)
    p.add_argument('--zero',action='store_true');p.add_argument('--output',type=pathlib.Path,required=True)
    a=p.parse_args();result=run(a.port,a.zero);private_write(a.output,result);print(json.dumps(result,indent=2))
