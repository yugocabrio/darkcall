#!/usr/bin/env python3
"""Loopback-only recorded demo and seller/bidder auction with optional regtest settlement.

Canonical synthetic and certified isolated-regtest runs are separate explicit
configurations. Rehearsal creates input proofs and signed synthetic acceptances;
it also clears those exact shares with REP3. Its explicit Settle action funds and
settles that SAME certified book on the isolated NU7 regtest through the existing
funded drivers. No spending keys, viewing keys, masks or raw memos are served to the browser.
"""
import argparse
import json
import os
import pathlib
import secrets
import threading
import time
from collections import defaultdict, deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit
from check_ticket_receipts import public_book
from open_ticket_memo import open_receipt
from regtest_deposits import ROOT, private_read
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call
from regtest_deposits import ENV, NATIVE
from public_bundle import verify as verify_bundle, snapshot
from rehearsal import Rehearsal

WEB = ROOT / 'demo'
DEFAULTS = {'positive':'positive-receipt-memo-20261005', 'zero':'zero-receipt-memo-v2-20261005'}
PUBLIC = ROOT / '.local/regtest-shared-v2-public-20261005'
AUTHORITY = ROOT / '.local/regtest-shared-v2-deposit4-20261005-admission/manifest.json'


def recorded_check(txid):
    """The committed clean-room result for this settlement, if any: a fresh Linux
    image built from committed source verified each public bundle, no network."""
    run = json.loads((ROOT / 'evidence/2026-10-18-clean-room-linux-verification.json').read_text())['run_head_rebuild']
    for entry in run['results']:
        if entry['txid'] == txid and run['all_passed']:
            return {'date':run['date'].split(' ')[0], 'revision':run['revision'], 'seconds':entry['seconds'],
                'record':'evidence/2026-10-18-clean-room-linux-verification.json',
                'price':entry['normal_cpp_auction_proof_verified'], 'payouts':entry['native_row_proofs_verified'],
                'transaction':entry['native_proof_and_signatures_verified'],
                'book':entry['accepted_book_and_recipients_authenticated']}
    return None


class Demo:
    def __init__(self, cases, canonical=None, bundle=None, public_only=False, funded=False, authority=None, note=None, chain='regtest'):
        self.funded = funded
        self.note = note
        self.authority = authority
        # The recorded auction's chain comes from its separately trusted
        # authority: the admission context names the genesis it was funded on.
        self.recorded_chain = 'regtest'
        if authority is not None:
            genesis = bytes(json.loads(pathlib.Path(authority).read_text())['context']['network_genesis']).hex()
            if genesis == '05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38':
                self.recorded_chain = 'testnet'
        if funded and (not canonical or authority is None):
            raise RuntimeError("Funded demo requires canonical run and separate trusted authority")
        self.canonical = canonical
        self.bundle = bundle
        self.public_only = public_only
        self.cases = {name: (ROOT / '.local' / path).resolve(strict=True) for name,path in cases.items()}
        local = (ROOT / '.local').resolve()
        if any(not p.is_relative_to(local) for p in self.cases.values()):
            raise RuntimeError('Demo fixtures must be ignored controlled checkpoints')
        self.token = secrets.token_urlsafe(32)
        # Two locks: one for the settled auction's checks (receipts, the public
        # verification, cheats), one for a new auction's jobs. A 20-minute
        # settlement must not block the settled page for every other visitor;
        # a check peaks near 0.6 GiB, a settlement near 3 GiB.
        self.lock = threading.Lock()
        self.rehearsal = Rehearsal(threading.Lock(), enabled=not public_only and bool(canonical), chain=chain)
        self.last_settlement = None
        self.ticket_order = None
        self.trail_cache = None

    def trail(self):
        # The four claims' evidence, from the committed public bundle and the
        # trusted authority only (scripts/proof_trail.py); never from .local.
        if not (self.canonical and self.funded):
            raise ValueError('No proof trail for this demo')
        if self.trail_cache is None:
            from proof_trail import trail
            self.trail_cache = trail(self.bundle, self.authority)
        return self.trail_cache

    def state(self):
        cases = []
        for name,path in self.cases.items():
            if self.canonical:
                if self.funded:
                    identity=json.loads(private_read(path/'nu7-native-identity.json'))
                    if identity['scope'] != 'actual isolated NU7 six-output auction preparation; no authorization':
                        raise RuntimeError('Funded demo native identity scope mismatch')
                    summary={'txid':identity['txid'],'branch':identity['branch'],'fee':30000,
                        'transaction_bytes':len(private_read(path/'funded-nu7-transaction.bin'))}
                    valid=len(identity['native_actions'])==6
                    if not self.public_only:
                        recorded=json.loads(private_read(path/'confirmed-summary.json',65536))
                        valid = (valid and recorded['full_node_accepted']
                            and recorded['canonical_confirmations'] >= 2
                            and recorded['construction']['native']['txid'] == identity['txid'])
                elif self.public_only:
                    identity=json.loads(private_read(path/'nu7-native-identity.json'))
                    summary={'txid':identity['txid'],'branch':identity['branch'],'fee':30000,
                        'transaction_bytes':len(private_read(path/'synthetic-nu7-transaction.bin')),
                        'chain_funding':False}
                    valid=len(identity['native_actions'])==6
                else:
                    recorded = json.loads(private_read(path.parent/(path.name+'-summary.json'),65536))['result']
                    summary = recorded['native']
                    valid = (recorded['passed'] and summary['four_receipt_memos_in_same_six_output_transaction']
                        and summary['synthetic_inputs'] == 4 and summary['actions'] == 6)
                book = json.loads(public_book(path/'receipt-book.json'))
                statement = private_read(path/'receipt-proof-public/public-input0',187*32)
            else:
                summary = json.loads(private_read(path / 'native-summary.json', 16384))
                book = json.loads(public_book(path / 'proof-public/book.json'))
                statement = private_read(path / 'proof-public/public-input0', 187*32)
                valid = summary.get('synthetic_inputs') == 1 and summary.get('actions') == 4
            if len(statement) != 187*32 or (not self.funded and summary.get('chain_funding') is not False) or not valid:
                raise RuntimeError('Demo receipt statement or recorded execution scope differs')
            price = int.from_bytes(statement[177*32:178*32], 'big')
            sold = int.from_bytes(statement[178*32:179*32], 'big')
            # Bidder numbers: the public book is sorted by ticket id; a recorded
            # sealing order (scripts/ticket_order.py) numbers them as the
            # browser did when each bid was sealed.
            ids = [bytes(ticket).hex() for ticket in book.get('ids', [])]
            labels = list(range(1, 5))
            if self.ticket_order is not None:
                order = self.ticket_order['sealing_order']
                if sorted(order) != sorted(ids) or self.ticket_order['txid'] != summary['txid']:
                    raise RuntimeError('Ticket order belongs to a different auction')
                labels = [order.index(ticket) + 1 for ticket in ids]
            cases.append({'id':name, 'price':price, 'sold':sold, 'units':3, 'tickets':4,
                'budget':book.get('budget'), 'labels':labels, 'labels_in_sealing_order':self.ticket_order is not None,
                'recorded_check':recorded_check(summary['txid']) if self.funded else None,
                'root':book['root'], 'txid':summary['txid'], 'fee':summary['fee'],
                'bytes':summary['transaction_bytes'], 'branch':summary['branch'],
                'scope':'recorded actual isolated NU7 certified funded memo auction' if self.funded else 'synthetic six-output auction with four receipt memos' if self.canonical else 'synthetic receipt component; separate one-input/four-output transfer'})
        return {'cases':cases, 'csrf':self.token, 'last_settlement':self.last_settlement,
            'funded_canonical':self.funded,'recorded_local_confirmations':2 if self.funded and not self.public_only else None,
            'recipient_identities_available':not self.public_only,
            'live_admission':False, 'local_rehearsal_available':self.rehearsal.enabled,
            'regtest_settlement_available':self.rehearsal.settlement_available,
            'live_chain':self.rehearsal.chain, 'recorded_chain':self.recorded_chain,
            'recorded_note':self.note,
            'independent_hosts':False, 'operators_know_results':True,
            'six_output_memo_e2e':bool(self.canonical), 'accepted_book_certified':False,
            'chain_inclusion_verified':False}

    def verify(self, name, ticket, tamper=False):
        if type(name) is not str or name not in self.cases or type(ticket) is not int or not 0 <= ticket < 4:
            raise ValueError('Unknown demo ticket')
        if self.public_only and not tamper:
            return 409, {'error':'This checkout has public proofs only. Receipt opening requires your private recipient identity.'}
        if not self.lock.acquire(blocking=False):
            return 409, {'error':'Another verification is running. Please try again shortly.'}
        try:
            source = self.cases[name]
            original_proof = source / ('receipt-proof-public' if self.canonical else 'proof-public')
            proof = original_proof
            nonce = secrets.token_hex(6)
            checkpoint = 'browser-receipt-' + nonce
            if tamper:
                proof = ROOT / '.local' / ('browser-tamper-' + nonce)
                proof.mkdir(mode=0o700)
                for file in ('vk', 'proof.0', 'public-input0'):
                    data = private_read(original_proof / file, 1048576)
                    if file == 'proof.0':
                        data = bytes([data[0] ^ 1]) + data[1:]
                    write_bytes(proof / file, data)
            try:
                if self.funded:
                    from funded_public_bundle import snapshot as funded_snapshot, export as funded_export
                    from open_funded_ticket import open_receipt as open_funded
                    bundle=self.bundle
                    if tamper:
                        from replay_nu7_auction import write_existing
                        frozen=funded_snapshot(bundle,self.authority,'browser-tamper-input')
                        path=frozen/'receipt-proof-public/proof.0'
                        data=private_read(path)
                        write_existing(path,bytes([data[0]^1])+data[1:])
                        forged=ROOT/'.local'/('browser-fundedtamper-'+nonce)
                        funded_export(frozen,forged)
                        bundle=forged
                    result=open_funded(source/'receipt-book.json',bundle,self.authority,
                        source/f'private-funded-recipient-{ticket}.json',ticket,checkpoint)
                    result.update(fixed_cpp_ticket_proof_verified=result['fixed_cpp_ticket_and_auction_proofs_verified'],
                        fixed_auction_context_proof_verified=True,receipt_in_six_output_transaction=True,
                        six_output_auction_settlement_verified=False,chain_inclusion_verified=False)
                else:
                    result = open_receipt(source/'receipt-book.json' if self.canonical else source/'proof-public/book.json', proof,
                        source if self.canonical else source/f'recipient-{ticket}', ticket, checkpoint,
                        auction_bundle=self.bundle if self.canonical else None)
            except RuntimeError as error:
                if tamper and str(error) == 'Fixed C++ ticket proof rejected before native decryption':
                    target = ROOT / '.local' / checkpoint
                    if ((target / f'private-recipient-{ticket}.json').exists()
                            or (target / f'private-auction-recipient-{ticket}.json').exists()
                            or (target / f'private-funded-recipient-{ticket}.json').exists()
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
                'six_output_auction_settlement_verified','chain_inclusion_verified',
                'receipt_in_six_output_transaction','fixed_auction_context_proof_verified')}
            public['ticket_index'] = ticket
            return 200, public
        finally:
            self.lock.release()

    def cheat(self, kind):
        # Forge a copy of the recorded public bundle one dishonest way and run the
        # fixed verifiers on it (scripts/cheat_lab.py). Public artifacts only.
        if not (self.canonical and self.funded):
            return 409, {'error':'The cheat lab needs the recorded funded settlement.'}
        from cheat_lab import CHEATS, run as run_cheat
        if kind not in CHEATS: raise ValueError('Unknown cheat')
        if not self.lock.acquire(blocking=False):
            return 409, {'error':'Another verification is running. Please try again shortly.'}
        try:
            return 200, run_cheat(self.bundle, self.authority, kind)
        finally:
            self.lock.release()

    def settlement(self):
        if not self.lock.acquire(blocking=False):
            return 409, {'error':'Another verification is running. Please try again shortly.'}
        try:
            if self.canonical:
                if self.funded:
                    from funded_public_bundle import verify as verify_funded
                    result=verify_funded(self.bundle,self.authority)
                else:
                    result = verify_bundle(self.bundle)
                public = {key:result[key] for key in ('txid','transaction_bytes','native_proof_and_signatures_verified',
                    'actual_transaction_outputs_match','seconds')}
                public.update(native_row_proofs_verified=result['application']['native_row_proofs_verified'],
                    normal_cpp_auction_proof_verified=result['application']['normal_cpp_auction_proof_verified'],
                    acceptance_or_recipient_receipts_authenticated=self.funded,private_openings_read=False,
                    chain_funding_or_finality_verified=False,
                    receipt_fixed_proof_verified=result['receipt_fixed_proof_verified'],
                    receipt_settlement_context_link_verified=result['receipt_settlement_context_link_verified'],
                    verified_at_unix=int(time.time()))
                if public['txid'] != self.state()['cases'][0]['txid']:
                    raise RuntimeError('Public verification is for a different demo auction')
                self.last_settlement=public
                return 200,public
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


# Hosted mode: requests per client address within a sliding window (seconds).
LIMITS = {'state':(1200,600),'verify':(12,600),'settlement':(4,600),'cheat':(8,600),'create':(3,3600),'rehearsal':(240,600)}


class RateLimit:
    def __init__(self):
        self.seen = defaultdict(deque); self.guard = threading.Lock()

    def allow(self, client, bucket):
        limit, window = LIMITS[bucket]; now = time.monotonic()
        with self.guard:
            hits = self.seen[(client, bucket)]
            while hits and hits[0] <= now - window: hits.popleft()
            if len(hits) >= limit: return False
            hits.append(now); return True


def handler(demo, port, public_origin=None):
    # Hosted: a TLS proxy on this machine forwards the configured public origin to
    # loopback. The server still binds loopback only and checks Host and Origin.
    host = urlsplit(public_origin).netloc if public_origin else '127.0.0.1:' + str(port)
    origin = public_origin.rstrip('/') if public_origin else 'http://' + host
    limits = RateLimit() if public_origin else None

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

        def limited(self, bucket):
            if limits is None: return False
            # Only the loopback proxy can connect, so its forwarded address is trusted.
            client = (self.headers.get('X-Forwarded-For') or 'proxy').split(',')[0].strip()
            if limits.allow(client, bucket): return False
            self.reply(429,{'error':'Too many requests from this address. Please try again in a few minutes.'})
            return True

        def do_GET(self):
            if not self.allowed_host(): return self.reply(403,{'error':'Loopback host required'})
            if self.headers.get('Origin') not in (None,origin):
                return self.reply(403,{'error':'Same origin required'})
            files = {'/':('index.html','text/html; charset=utf-8'),
                '/demo/settled':('index.html','text/html; charset=utf-8'),
                '/demo/run':('index.html','text/html; charset=utf-8'),
                '/app.js':('app.js','text/javascript; charset=utf-8'),
                '/style.css':('style.css','text/css; charset=utf-8'),
                '/media/darkcall-demo-v2.mp4':('media/v2/darkcall-demo-v2.mp4','video/mp4'),
                # Site icon: the gavel. Named for it, so browsers that cached the
                # earlier diamond fetch it afresh; they also ask for /favicon.ico.
                '/icon-gavel.svg':('icons/favicon.svg','image/svg+xml'),
                '/icon-gavel-32.png':('icons/favicon-32.png','image/png'),
                '/icon-gavel-180.png':('icons/apple-touch-icon.png','image/png'),
                '/favicon.ico':('icons/favicon-32.png','image/png'),
                '/apple-touch-icon.png':('icons/apple-touch-icon.png','image/png')}
            # Fixed self-hosted fonts (IBM Plex, SIL OFL); no path is derived from the request.
            files.update({f'/fonts/{name}':(f'fonts/{name}','font/woff2') for name in (
                'ibm-plex-sans-latin-400-normal.woff2','ibm-plex-sans-latin-500-normal.woff2',
                'ibm-plex-sans-latin-600-normal.woff2','ibm-plex-mono-latin-400-normal.woff2',
                'ibm-plex-mono-latin-500-normal.woff2')})
            if self.path in files:
                name,kind = files[self.path]
                return self.reply(200,(WEB / name).read_bytes(),kind)
            if self.path.startswith('/api/') and self.limited('state'): return
            if self.path == '/api/state':
                try: return self.reply(200,{**demo.state(),'hosted':public_origin is not None})
                except Exception: return self.reply(503,{'error':'Demo checkpoints unavailable'})
            if self.path == '/api/trail':
                try: return self.reply(200,demo.trail())
                except ValueError: return self.reply(404,{'error':'No proof trail for this demo'})
                except Exception: return self.reply(503,{'error':'Public bundle unavailable'})
            if self.path.startswith('/api/rehearsal/auction/'):
                try: return self.reply(200,demo.rehearsal.get(self.path.removeprefix('/api/rehearsal/auction/')))
                except ValueError: return self.reply(404,{'error':'Unknown local demo auction'})
            return self.reply(404,{'error':'Not found'})

        def do_POST(self):
            if (not self.allowed_host() or self.headers.get('Origin') != origin
                    or self.headers.get('X-DarkCall-Token') != demo.token):
                return self.reply(403,{'error':'Same origin and session token required'})
            rehearsal_actions={'/api/rehearsal/create':'create','/api/rehearsal/bid':'bid','/api/rehearsal/close':'close',
                '/api/rehearsal/clear':'clear','/api/rehearsal/result':'result','/api/rehearsal/fund':'fund','/api/rehearsal/settle':'settle',
                '/api/rehearsal/cancel':'cancel'}
            if self.path not in ('/api/verify','/api/settlement','/api/cheat',*rehearsal_actions): return self.reply(404,{'error':'Not found'})
            bucket = {'/api/verify':'verify','/api/settlement':'settlement','/api/cheat':'cheat','/api/rehearsal/create':'create'}.get(self.path,'rehearsal')
            if self.limited(bucket): return
            try:
                length = int(self.headers.get('Content-Length','0'))
                if not 0 < length <= 1024 or self.headers.get('Content-Type') != 'application/json':
                    return self.reply(400,{'error':'Bounded JSON request required'})
                value = json.loads(self.rfile.read(length))
                if self.path in rehearsal_actions:
                    try: status,result=demo.rehearsal.post(rehearsal_actions[self.path],value)
                    except ValueError as error: return self.reply(400,{'error':str(error)})
                    return self.reply(status,result)
                if self.path == '/api/cheat':
                    if not isinstance(value,dict) or set(value) != {'kind'} or not isinstance(value['kind'],str):
                        return self.reply(400,{'error':'Expected one cheat kind'})
                    status,result = demo.cheat(value['kind'])
                    return self.reply(status,result)
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
    parser.add_argument('--canonical',action='store_true',help='One completed NU7 six-output memo auction; no mixed fixtures')
    parser.add_argument('--run',default='canonical-nu7-memos-20261005')
    parser.add_argument('--bundle',type=pathlib.Path,default=ROOT/'demo/artifacts/canonical-nu7-v1')
    parser.add_argument('--public-only',action='store_true',help='Use only committed public artifacts; no synthetic identities')
    parser.add_argument('--funded',action='store_true',help='Recorded certified actual-note isolated NU7 run')
    parser.add_argument('--authority',type=pathlib.Path,help='Separate caller-trusted admission manifest')
    parser.add_argument('--recorded-note',help='Short public description of the recorded auction')
    parser.add_argument('--ticket-order',type=pathlib.Path,help='Recorded sealing order of the tickets (demo/ticket-order)')
    parser.add_argument('--public-origin',help='Hosted mode: the HTTPS origin a local TLS proxy forwards to this loopback server')
    parser.add_argument('--chain',choices=('regtest','testnet'),default='regtest',
                        help='Where Fund and Settle run; testnet uses public testnet coins through the pinned provider')
    args = parser.parse_args()
    if args.funded and (not args.canonical or args.authority is None):
        parser.error('--funded requires --canonical and explicit separate --authority')
    if not 1024 <= args.port <= 65535: raise RuntimeError('Expected unprivileged local port')
    if args.public_origin is not None:
        parts = urlsplit(args.public_origin)
        if parts.scheme != 'https' or not parts.netloc or parts.path not in ('','/') or parts.query or parts.fragment:
            parser.error('--public-origin must be a bare https origin, for example https://demo.example.org')
    if args.canonical:
        # CLI configuration is trusted local setup, never an HTTP supplied path.
        bundle = args.bundle.resolve(strict=True)
        run=ROOT/'.local'/args.run
        if args.public_only or not run.exists():
            if args.funded:
                from funded_public_bundle import snapshot as funded_snapshot
                frozen=funded_snapshot(bundle,args.authority,'funded-demo-public')
            else:
                frozen=snapshot(bundle,'demo-public')
            demo=Demo({'positive':frozen.name},canonical=True,bundle=bundle,public_only=True,
                funded=args.funded,authority=args.authority,note=args.recorded_note)
        else:
            demo = Demo({'positive':args.run},canonical=args.run,bundle=bundle,funded=args.funded,authority=args.authority,
                note=args.recorded_note,chain=args.chain)
    else:
        demo = Demo({'positive':args.positive,'zero':args.zero})
    if args.ticket_order is not None:
        demo.ticket_order = json.loads(args.ticket_order.read_text())
    demo.state()  # Fail before binding if fixtures are missing or wrong scope.
    if args.public_origin and demo.rehearsal.enabled: demo.rehearsal.prune_after = 3 * 3600
    server = ThreadingHTTPServer(('127.0.0.1',args.port),handler(demo,args.port,args.public_origin))
    server.timeout = 2
    print(f'DarkCall demo: http://127.0.0.1:{args.port}' + (f' (hosted as {args.public_origin})' if args.public_origin else ''),flush=True)
    try: server.serve_forever()
    finally: server.server_close()


if __name__ == '__main__':
    main()
