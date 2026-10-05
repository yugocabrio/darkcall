#!/usr/bin/env python3
"""Loopback-only receipt demo running the actual fixed-proof/native client.

All fixtures are synthetic. This is a local presentation of the completed
one-input/four-output receipt component, not live admission or six-output E2E.
No spending keys, viewing keys, masks or raw memos are served to the browser.
"""
import argparse
import json
import os
import pathlib
import secrets
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from check_ticket_receipts import public_book
from open_ticket_memo import open_receipt
from regtest_deposits import ROOT, private_read
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call
from regtest_deposits import ENV, NATIVE

WEB = ROOT / 'demo'
DEFAULTS = {'positive':'positive-receipt-memo-20261005', 'zero':'zero-receipt-memo-v2-20261005'}
PUBLIC = ROOT / '.local/regtest-shared-v2-public-20261005'
AUTHORITY = ROOT / '.local/regtest-shared-v2-deposit4-20261005-admission/manifest.json'


class Demo:
    def __init__(self, cases):
        self.cases = {name: (ROOT / '.local' / path).resolve(strict=True) for name,path in cases.items()}
        local = (ROOT / '.local').resolve()
        if any(not p.is_relative_to(local) for p in self.cases.values()):
            raise RuntimeError('Demo fixtures must be ignored synthetic checkpoints')
        self.token = secrets.token_urlsafe(32)
        self.lock = threading.Lock()
        self.last_settlement = None

    def state(self):
        cases = []
        for name,path in self.cases.items():
            summary = json.loads(private_read(path / 'native-summary.json', 16384))
            book = json.loads(public_book(path / 'proof-public/book.json'))
            statement = private_read(path / 'proof-public/public-input0', 187*32)
            if (len(statement) != 187*32 or summary.get('chain_funding') is not False
                    or summary.get('synthetic_inputs') != 1 or summary.get('actions') != 4):
                raise RuntimeError('Demo accepts only the documented synthetic receipt component')
            price = int.from_bytes(statement[177*32:178*32], 'big')
            sold = int.from_bytes(statement[178*32:179*32], 'big')
            cases.append({'id':name, 'price':price, 'sold':sold, 'units':3, 'tickets':4,
                'root':book['root'], 'txid':summary['txid'], 'fee':summary['fee'],
                'bytes':summary['transaction_bytes'], 'branch':summary['branch'],
                'scope':'synthetic receipt component; separate one-input/four-output transfer'})
        return {'cases':cases, 'csrf':self.token, 'last_settlement':self.last_settlement,
            'live_admission':False, 'independent_hosts':False, 'operators_know_results':True,
            'six_output_memo_e2e':False, 'chain_inclusion_verified':False}

    def verify(self, name, ticket, tamper=False):
        if type(name) is not str or name not in self.cases or type(ticket) is not int or not 0 <= ticket < 4:
            raise ValueError('Unknown demo ticket')
        if not self.lock.acquire(blocking=False):
            return 409, {'error':'Another verification is running. Please try again shortly.'}
        try:
            source = self.cases[name]
            proof = source / 'proof-public'
            nonce = secrets.token_hex(6)
            checkpoint = 'browser-receipt-' + nonce
            if tamper:
                proof = ROOT / '.local' / ('browser-tamper-' + nonce)
                proof.mkdir(mode=0o700)
                for file in ('vk', 'proof.0', 'public-input0'):
                    data = private_read(source / 'proof-public' / file, 1048576)
                    if file == 'proof.0':
                        data = bytes([data[0] ^ 1]) + data[1:]
                    write_bytes(proof / file, data)
            try:
                result = open_receipt(source / 'proof-public/book.json', proof,
                    source / f'recipient-{ticket}', ticket, checkpoint)
            except RuntimeError as error:
                if tamper and str(error) == 'Fixed C++ ticket proof rejected before native decryption':
                    target = ROOT / '.local' / checkpoint
                    if ((target / f'private-recipient-{ticket}.json').exists()
                            or (target / 'private-received.memo').exists()):
                        raise RuntimeError('Tamper gate unexpectedly accessed recipient key')
                    return 200, {'rejected':True, 'key_read':False, 'memo_decrypted':False,
                        'reason':'Changed proof rejected by the fixed verifier before the recipient key was read.'}
                raise
            if tamper:
                raise RuntimeError('Changed proof unexpectedly accepted')
            # Explicit allowlist: no private artifact paths, key bytes or pads.
            public = {key:result[key] for key in ('won','refund','price','sold','txid','seconds',
                'fixed_cpp_ticket_proof_verified','native_proof_and_signatures_verified',
                'own_ticket_opening_verified','native_amount_matches_refund','own_viewing_key_only',
                'six_output_auction_settlement_verified','chain_inclusion_verified')}
            public['ticket_index'] = ticket
            return 200, public
        finally:
            self.lock.release()

    def settlement(self):
        if not self.lock.acquire(blocking=False):
            return 409, {'error':'Another verification is running. Please try again shortly.'}
        try:
            # Fixed public-only fixture and independently configured authority.
            # Never discover authority keys in a supplied proof bundle or accept
            # a browser-provided path. Snapshot exact public bytes before verify.
            target = ROOT / '.local' / ('browser-settlement-' + secrets.token_hex(6))
            target.mkdir(mode=0o700)
            mpc = target / 'mpc-public'
            mpc.mkdir(mode=0o700)
            for file in ['native-public.json','native-layout.json','final-book-certificate.json',
                    'regtest-transaction.bin'] + [f'native-binding-proof-{i}' for i in range(6)]:
                write_bytes(target / file, private_read(PUBLIC / file,1000000))
            original = json.loads(private_read(PUBLIC / 'mpc-result.json',4096))
            source = pathlib.Path(original['run']).resolve(strict=True)
            if source != (PUBLIC / 'mpc-public').resolve(strict=True):
                raise RuntimeError('Expected public-only settlement checkpoint')
            for file in ('vk','proof.0','public-input0','tampered-price','tampered-plan','tampered-row'):
                write_bytes(mpc / file,private_read(source / file,1000000))
            original['run'] = str(mpc)
            write_bytes(target / 'mpc-result.json',json.dumps(original).encode())
            # Explicit authority comes from the original registration setup,
            # outside the public proof bundle, and contains public keys only.
            authority = ROOT / '.local' / (target.name + '-authority.json')
            write_bytes(authority,private_read(AUTHORITY,32768))
            result = guarded_json_call([str(NATIVE),'--verify-certified-integrated',str(target),str(authority)],
                cwd=ROOT,env=ENV,timeout=900,observations=target/'verification-resources.jsonl')
            if (not all(result[key] for key in ('native_proof_and_signatures_verified',
                    'native_row_proofs_verified','normal_cpp_auction_proof_verified',
                    'actual_transaction_outputs_match','acceptance_or_recipient_receipts_authenticated'))
                    or result['private_openings_read'] or result['chain_funding_or_finality_verified']):
                raise RuntimeError('Incomplete public settlement verification')
            public = {key:result[key] for key in ('txid','transaction_bytes','native_proof_and_signatures_verified',
                'native_row_proofs_verified','normal_cpp_auction_proof_verified','actual_transaction_outputs_match',
                'acceptance_or_recipient_receipts_authenticated','private_openings_read',
                'chain_funding_or_finality_verified','seconds')}
            public['verified_at_unix'] = int(time.time())
            write_bytes(target / 'verification-result.json',json.dumps(public).encode())
            self.last_settlement = public
            return 200, public
        finally:
            self.lock.release()


def handler(demo, port):
    host = '127.0.0.1:' + str(port)
    origin = 'http://' + host

    class Handler(BaseHTTPRequestHandler):
        def reply(self, status, data, kind='application/json'):
            if kind == 'application/json': data = json.dumps(data).encode()
            self.send_response(status)
            self.send_header('Content-Type',kind)
            self.send_header('Content-Length',str(len(data)))
            self.send_header('Cache-Control','no-store')
            self.send_header('X-Content-Type-Options','nosniff')
            self.send_header('X-Frame-Options','DENY')
            self.send_header('Referrer-Policy','no-referrer')
            self.send_header('Content-Security-Policy',"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
            self.end_headers()
            self.wfile.write(data)

        def allowed_host(self):
            return self.client_address[0] == '127.0.0.1' and self.headers.get('Host') == host

        def do_GET(self):
            if not self.allowed_host(): return self.reply(403,{'error':'Loopback host required'})
            if self.headers.get('Origin') not in (None,origin):
                return self.reply(403,{'error':'Same origin required'})
            files = {'/':('index.html','text/html; charset=utf-8'),
                '/app.js':('app.js','text/javascript; charset=utf-8'),
                '/style.css':('style.css','text/css; charset=utf-8')}
            if self.path in files:
                name,kind = files[self.path]
                return self.reply(200,(WEB / name).read_bytes(),kind)
            if self.path == '/api/state':
                try: return self.reply(200,demo.state())
                except Exception: return self.reply(503,{'error':'Demo checkpoints unavailable'})
            return self.reply(404,{'error':'Not found'})

        def do_POST(self):
            if (not self.allowed_host() or self.headers.get('Origin') != origin
                    or self.headers.get('X-DarkCall-Token') != demo.token):
                return self.reply(403,{'error':'Same origin and session token required'})
            if self.path not in ('/api/verify','/api/settlement'): return self.reply(404,{'error':'Not found'})
            try:
                length = int(self.headers.get('Content-Length','0'))
                if not 0 < length <= 1024 or self.headers.get('Content-Type') != 'application/json':
                    return self.reply(400,{'error':'Bounded JSON request required'})
                value = json.loads(self.rfile.read(length))
                if self.path == '/api/settlement':
                    if value != {}: return self.reply(400,{'error':'Settlement accepts no supplied paths or keys'})
                    status,result = demo.settlement()
                    return self.reply(status,result)
                if (not isinstance(value,dict) or set(value) != {'case','ticket','action'}
                        or value['action'] not in ('verify','tamper')):
                    return self.reply(400,{'error':'Expected a demo case, ticket and action'})
                status,result = demo.verify(value['case'],value['ticket'],value['action']=='tamper')
                return self.reply(status,result)
            except ValueError:
                return self.reply(400,{'error':'Invalid demo request'})
            except Exception:
                # Never publish a subprocess diagnostic/private path on the UI.
                return self.reply(422,{'error':'Verification stopped. Inspect the retained local checkpoint.'})

        def log_message(self, *_):
            pass

    return Handler


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port',type=int,default=8877)
    parser.add_argument('--positive',default=DEFAULTS['positive'])
    parser.add_argument('--zero',default=DEFAULTS['zero'])
    args = parser.parse_args()
    if not 1024 <= args.port <= 65535: raise RuntimeError('Expected unprivileged local port')
    demo = Demo({'positive':args.positive,'zero':args.zero})
    demo.state()  # Fail before binding if fixtures are missing or wrong scope.
    server = ThreadingHTTPServer(('127.0.0.1',args.port),handler(demo,args.port))
    server.timeout = 2
    print(f'DarkCall local receipt demo: http://127.0.0.1:{args.port}',flush=True)
    try: server.serve_forever()
    finally: server.server_close()


if __name__ == '__main__':
    main()
