#!/usr/bin/env python3
"""Actual loopback demo API checks, including key-before-proof refusal.

Synthetic fixtures only. HTTP checks do not prove MPC transport security or
independent operator separation. Responses are recursively checked for secrets.
"""
import argparse
import http.client
import json
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8877)
    parser.add_argument('--settlement-security-only', action='store_true')
    parser.add_argument('--settlement-only', action='store_true')
    parser.add_argument('--canonical',action='store_true')
    parser.add_argument('--public-only',action='store_true')
    args = parser.parse_args()
    host = '127.0.0.1:' + str(args.port)
    origin = 'http://' + host
    start = time.monotonic()

    def call(method, path, body=None, headers=None):
        connection = http.client.HTTPConnection('127.0.0.1',args.port,timeout=950 if args.settlement_only else 120)
        try:
            connection.request(method,path,body,headers or {})
            response = connection.getresponse()
            data = response.read(100000)
            if response.getheader('Cache-Control') != 'no-store': raise RuntimeError('Missing private response cache rule')
            if response.getheader('X-Frame-Options') != 'DENY': raise RuntimeError('Missing frame isolation')
            return response.status, data
        finally: connection.close()

    status,raw = call('GET','/api/state')
    if status != 200: raise RuntimeError('Demo state unavailable')
    state = json.loads(raw)
    if args.canonical and (not state['six_output_memo_e2e'] or len(state['cases']) != 1):
        raise RuntimeError('Expected one same-run six-output auction')
    headers = {'Origin':origin,'Content-Type':'application/json','X-DarkCall-Token':state['csrf']}
    if args.settlement_only:
        status,data = call('POST','/api/settlement','{}',headers)
        value = json.loads(data)
        if (status != 200 or not all(value[k] for k in ('native_proof_and_signatures_verified',
                'native_row_proofs_verified','normal_cpp_auction_proof_verified','actual_transaction_outputs_match',
                *(() if args.canonical else ('acceptance_or_recipient_receipts_authenticated',))))
                or value['private_openings_read'] or value['chain_funding_or_finality_verified']):
            raise RuntimeError('Public settlement API verification did not pass')
        if args.canonical and (value['txid'] != state['cases'][0]['txid']
                or not value['receipt_fixed_proof_verified'] or not value['receipt_settlement_context_link_verified']
                or value['acceptance_or_recipient_receipts_authenticated']):
            raise RuntimeError('Canonical settlement scope/receipt transaction differs')
        forbidden = {'viewing_key','spending_key','refund_pad','winner_pad','pczt','witness','private_path'}
        if forbidden.intersection(value): raise RuntimeError('Private field in settlement response')
        print(json.dumps({'scope':'actual public-only same-run six-output memo verification' if args.canonical else 'actual public-only six-output settlement API verification; fixed historical regtest bundle',
            'result':value,'private_fields_served':False,'no_proving_key_or_new_signature':True,
            'seconds':time.monotonic()-start},indent=2))
        return
    if args.settlement_security_only:
        checks = []
        for label,value,extra,expected in [
            ('supplied-path',{'path':'../../wallet'}, {},400),
            ('supplied-authority',{'authority':'untrusted'}, {},400),
            ('supplied-key',{'viewing_key':'unused'}, {},400),
            ('cross-origin',{}, {'Origin':'https://attacker.invalid'},403),
            ('wrong-session-token',{}, {'X-DarkCall-Token':'invalid'},403)]:
            if call('POST','/api/settlement',json.dumps(value),{**headers,**extra})[0] != expected:
                raise RuntimeError('Settlement API boundary rejection absent: '+label)
            checks.append(label)
        print(json.dumps({'scope':'public settlement API boundary checks; no extra proof verification run',
            'rejected':checks,'private_paths_or_supplied_keys_accepted':False,'seconds':time.monotonic()-start},indent=2))
        return
    denied = []
    for label,path,extra,expected in [
        ('private-file','/.local/private-recipient-0.json',{},404),
        ('path-traversal','/../.local/private-recipient-0.json',{},404),
        ('arbitrary-evidence','/api/evidence/../wallet',{},404),
        ('dns-rebinding','/api/state',{'Host':'attacker.invalid'},403),
        ('cross-origin-read','/api/state',{'Origin':'https://attacker.invalid'},403)]:
        if call('GET',path,headers=extra)[0] != expected: raise RuntimeError('Missing HTTP rejection: '+label)
        denied.append(label)
    payload = json.dumps({'case':'positive','ticket':0,'action':'verify'})
    for label,extra,expected in [
        ('missing-origin',{'Origin':''},403),('cross-origin-write',{'Origin':'https://attacker.invalid'},403),
        ('missing-token',{'X-DarkCall-Token':''},403),('wrong-token',{'X-DarkCall-Token':'invalid'},403),
        ('wrong-content-type',{'Content-Type':'text/plain'},400)]:
        if call('POST','/api/verify',payload,{**headers,**extra})[0] != expected: raise RuntimeError('Missing HTTP rejection: '+label)
        denied.append(label)
    for label,payload in [
        ('arbitrary-path',{'case':'../../wallet','ticket':0,'action':'verify'}),
        ('unknown-action',{'case':'positive','ticket':0,'action':'sign'}),
        ('boolean-ticket',{'case':'positive','ticket':True,'action':'verify'}),
        ('unknown-ticket',{'case':'positive','ticket':4,'action':'verify'}),
        ('extra-secret-field',{'case':'positive','ticket':0,'action':'verify','viewing_key':'unused'})]:
        if call('POST','/api/verify',json.dumps(payload),headers)[0] != 400: raise RuntimeError('Missing input rejection: '+label)
        denied.append(label)
    forbidden = {'viewing_key','spend','spending_key','refund_pad','winner_pad','masks','blind','pczt','memo','private_path'}
    def clean(value):
        if isinstance(value,dict):
            if forbidden.intersection(value): raise RuntimeError('Private field leaked in API response')
            for item in value.values(): clean(item)
        elif isinstance(value,list):
            for item in value: clean(item)
    clean(state)
    if args.public_only:
        if state['recipient_identities_available']: raise RuntimeError('Public-only view loaded private identities')
        payload={'case':'positive','ticket':0,'action':'verify'}
        if call('POST','/api/verify',json.dumps(payload),headers)[0] != 409:
            raise RuntimeError('Missing own identity did not fail closed')
        payload['action']='tamper'
        status,data=call('POST','/api/verify',json.dumps(payload),headers)
        value=json.loads(data)
        if status!=200 or not value['rejected'] or value['key_read'] or value['memo_decrypted']:
            raise RuntimeError('Public-only proof tamper refusal failed')
        print(json.dumps({'scope':'public-only canonical demo; no private identity fixtures',
            'recipient_keys_included':False,'own_opening_without_key_refused':True,
            'proof_tamper_refused_before_key':True,'http_rejections':denied,
            'seconds':time.monotonic()-start},indent=2))
        return
    verified = []
    cases=[('positive',0,True,50),('positive',1,True,50),('positive',2,True,50),('positive',3,False,100)] if args.canonical else [('positive',0,True,50),('positive',3,False,100),('zero',0,True,100),('zero',3,False,100)]
    for name,index,won,refund in cases:
        status,data = call('POST','/api/verify',json.dumps({'case':name,'ticket':index,'action':'verify'}),headers)
        value = json.loads(data)
        if (status != 200 or value['won'] != won or value['refund'] != refund
                or not value['fixed_cpp_ticket_proof_verified'] or not value['native_proof_and_signatures_verified']
                or not value['own_ticket_opening_verified'] or value['six_output_auction_settlement_verified']
                or value['chain_inclusion_verified']):
            raise RuntimeError('Actual receipt API result differs from scoped fixture')
        if args.canonical and (value['txid'] != state['cases'][0]['txid']
                or not value['receipt_in_six_output_transaction'] or not value['fixed_auction_context_proof_verified']):
            raise RuntimeError('Receipt is from a different auction')
        clean(value)
        verified.append({'case':name,'ticket':index,'won':won,'refund':refund})
    for name in ('positive',) if args.canonical else ('positive','zero'):
        status,data = call('POST','/api/verify',json.dumps({'case':name,'ticket':0,'action':'tamper'}),headers)
        value = json.loads(data)
        if status != 200 or not value['rejected'] or value['key_read'] or value['memo_decrypted']:
            raise RuntimeError('HTTP corrupted-proof gate did not preserve recipient secrecy')
        clean(value)
    print(json.dumps({'scope':'loopback receipt demo actual API/client integration',
        'verified':verified,'http_rejections':denied,'corrupt_proofs_rejected_before_key':2,
        'private_fields_served':False,'browser_ui_checked_separately':True,
        'six_output_memo_e2e_verified':args.canonical,'seconds':time.monotonic()-start},indent=2))


if __name__ == '__main__':
    main()
