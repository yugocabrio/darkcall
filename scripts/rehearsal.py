"""Fresh LOCAL seller/bidder rehearsal and optional same-book regtest settlement.

The localhost client helper sees typed demo bids. It generates the existing
Noir ZK input proof and REP3 shares, then checks the proof/pairs before issuing
three signed synthetic receipts, followed by certified REP3 clearing. An explicit
Settle action then funds the SAME certified book with real isolated NU7 regtest
deposits from a local test bank and runs the existing funded settlement driver.
Never real bids or public-chain funds. Restart starts a new rehearsal namespace;
ignored artifacts are retained, never auto-resumed.
"""
import hashlib
import hmac
import json
import os
import pathlib
import re
import secrets
import subprocess
import sys
import threading
import time
from regtest_deposits import ROOT, private_read
from regtest_resources import check, sample, guarded_json_call
from run_mpc_gate import INPUT_TOOL, PLAIN, CLIENT_CIRCUIT, UPSTREAM, PIN, BIN
from verify_external import BB, SOURCE, CRS, PIN as BB_PIN

HELPER=ROOT/'target/release/examples/rehearsal_admission'
SETTLE=ROOT/'scripts/settle_fresh_book.py'
ID=re.compile(r'[0-9a-f]{32}\Z')
# Public progress fields the settlement driver records; anything else stays private.
SETTLE_FIELDS={'phase','state','elapsed_seconds','at_unix','height','accepted_root','deposit_txid',
    'confirmations','client_proofs','share_pairs','receipt_signatures','final_book_signatures',
    'same_accepted_root','txid','price','sold','recipient_scans','seconds',
    'native_row_proofs_verified','accepted_book_certified'}


def standard_terms(a):
    # The existing native funded path is pinned to these prototype terms.
    return a['units']==3 and a['budget']==100 and a['reserve'] in (0,20)


def number(value,minimum,maximum):
    if type(value) is not int or not minimum<=value<=maximum:
        raise ValueError('Use bounded whole-number demo amounts and counts')
    return value


class Rehearsal:
    def __init__(self,lock,enabled=True):
        self.lock=lock
        self.enabled=enabled and all(p.is_file() for p in (HELPER,INPUT_TOOL,PLAIN,CLIENT_CIRCUIT,BB))
        self.clearing_available=self.enabled and BIN.is_file()
        from settle_fresh_book import REQUIRED, BANK
        self.settlement_available=(self.clearing_available and (BANK/'nu7-birthday.json').is_file()
            and all(p.is_file() for p in REQUIRED))
        self.session=None; self.auctions={}

    def call(self,command,folder,label,secret_input=None,parse=False):
        check(sample(ROOT))
        logfile=folder/(label+'.log')
        with logfile.open('xb') as log:
            result=subprocess.run(list(map(str,command)),input=secret_input,cwd=ROOT,
                env={**os.environ,'RAYON_NUM_THREADS':'2'},stdout=subprocess.PIPE if parse else log,
                stderr=log,timeout=180)
        if result.returncode: raise RuntimeError('Local rehearsal gate stopped; private checkpoint retained')
        return json.loads(result.stdout) if parse else None

    def initialize(self):
        if self.session is not None: return
        if subprocess.check_output(['git','-C',str(UPSTREAM),'rev-parse','HEAD']).decode().strip()!=PIN:
            raise RuntimeError('Pinned input prover unavailable')
        if subprocess.check_output(['git','-C',str(SOURCE),'rev-parse','HEAD']).decode().strip()!=BB_PIN:
            raise RuntimeError('Pinned fixed verifier unavailable')
        patches=(ROOT/'patches/barretenberg-apple-clang15.patch').read_bytes()+(ROOT/'patches/barretenberg-macos-ipc.patch').read_bytes()
        if subprocess.check_output(['git','-C',str(SOURCE),'-c','core.abbrev=7','diff','--'])!=patches:
            raise RuntimeError('Fixed verifier source changed')
        local=ROOT/'.local'
        if local.is_symlink(): raise RuntimeError('Local directory symlink refused')
        session=local/('browser-rehearsal-'+secrets.token_hex(8)); session.mkdir(mode=0o700)
        vk=session/'trusted-vk'; vk.mkdir(mode=0o700)
        self.call([BB,'write_vk','--scheme','ultra_honk','--verifier_target','noir-recursive','-c',CRS,'-b',CLIENT_CIRCUIT,'-o',vk],session,'trusted-vk')
        self.session=session

    def public(self,a):
        return {'auction_id':a['id'],'units':a['units'],'reserve':a['reserve'],'budget':a['budget'],
            'processing_fee':15000,'deposit':a['budget']+15000,'closes_at_utc':a['closes_at_utc'],
            'accepted':len(a['receipts']),'capacity':4,'closed':a['certificate'] is not None,
            'stopped_after_incomplete_admission':a['poisoned'],
            'cutoff_reached':int(time.time())>=a['closes_at_utc'],'authority':a['authority'],
            'condition_digest':a['digest'],'accepted_root':a['certificate']['accepted_root'] if a['certificate'] else None,
            'funding_scope':'offline-synthetic','client_helper_reads_demo_bid':True,
            'chain_funding_verified':False,'fresh_clearing_or_settlement':a['clearing'] is not None,
            'fresh_clearing_verified':a['clearing'] is not None,'native_settlement_started':a['settlement'] is not None,
            'clearing_available':self.clearing_available,'clearing':a['clearing'],
            'clearing_stopped':a['clearing_failed'],
            'settlement_available':self.settlement_available and standard_terms(a),
            'settlement':self.settlement_view(a)}

    def settlement_view(self,a):
        job=a['settlement']
        if job is None: return None
        events=[]
        try:
            for line in private_read(job['progress'],1048576).decode().splitlines():
                value=json.loads(line); events.append({k:v for k,v in value.items() if k in SETTLE_FIELDS})
        except FileNotFoundError: pass
        view={'state':job['state'],'started_at_unix':job['started_at_unix'],'events':events,
            'funding_scope':'actual isolated NU7 regtest deposits from a local test bank',
            'public_testnet':False,'result':job['public']}
        if job['state']=='failed': view['error']='Settlement stopped. Its private checkpoint is retained.'
        return view

    def lookup(self,identifier):
        if type(identifier) is not str or not ID.fullmatch(identifier) or identifier not in self.auctions:
            raise ValueError('Unknown local demo auction')
        return self.auctions[identifier]

    def get(self,identifier):
        return self.public(self.lookup(identifier))

    def create(self,value):
        if type(value) is not dict or set(value)!={'units','reserve','budget','closes_at_utc'}:
            raise ValueError('Expected units, reserve, deposit budget and UTC close time')
        now=int(time.time())
        units=number(value['units'],1,3); budget=number(value['budget'],0,1000000)
        reserve=number(value['reserve'],0,budget)
        cutoff=number(value['closes_at_utc'],now+30,now+86400)
        if len(self.auctions)>=8: raise ValueError('Eight local demo auctions per server session maximum')
        self.initialize()
        identifier=secrets.token_hex(16)
        digest=hashlib.sha256(b'DarkCall/browser-rehearsal/conditions/v1\0'+bytes.fromhex(identifier)
            +units.to_bytes(4,'little')+reserve.to_bytes(8,'little')+budget.to_bytes(8,'little')
            +(15000).to_bytes(8,'little')+cutoff.to_bytes(8,'little')).digest()
        path=self.session/identifier; path.mkdir(mode=0o700)
        request={'condition_digest':list(digest),'units':units,'reserve':reserve,'budget':budget,'closes_at_utc':cutoff}
        initialized=self.call([HELPER,'init',path],path,'init',json.dumps(request).encode(),parse=True)
        a={'id':identifier,'units':units,'reserve':reserve,'budget':budget,'closes_at_utc':cutoff,
            'digest':list(digest),'path':path,'authority':initialized['authority'],'receipts':[],
            'clients':[],'opening_tokens':{},'certificate':None,'poisoned':False,
            'clearing':None,'clearing_run':None,'clearing_failed':False,'settlement':None}
        self.auctions[identifier]=a
        return self.public(a)

    def bid(self,value):
        if type(value) is not dict or set(value)!={'auction_id','maximum_bid'}:
            raise ValueError('Expected auction ID and maximum demo bid')
        a=self.lookup(value['auction_id']); maximum=number(value['maximum_bid'],0,a['budget'])
        if a['poisoned']: raise ValueError('Auction stopped after an incomplete admission; start a new rehearsal')
        if int(time.time())>=a['closes_at_utc'] or a['certificate']: raise ValueError('Bidding has closed')
        i=len(a['receipts'])
        if i>=4: raise ValueError('All four demo tickets are accepted')
        folder=a['path']/('client-'+secrets.token_hex(8)); started=time.monotonic()
        ticket=secrets.token_bytes(32)
        request={'condition_digest':a['digest'],'ticket':list(ticket),'bidder':i,'units':a['units'],
            'reserve':a['reserve'],'budget':a['budget'],'processing_fee':15000,'bid':maximum}
        # prepare-one refuses an existing directory; keep input off argv/logs.
        self.call([INPUT_TOOL,'prepare-one',folder],a['path'],'prepare-'+folder.name,json.dumps(request).encode())
        self.call([PLAIN,'--prover-crs',CRS/'bn254_g1.dat','--verifier-crs',CRS/'bn254_g2.dat',
            '--input',folder/'Prover.toml','--circuit',CLIENT_CIRCUIT,'--hasher','poseidon2','--out-dir',folder,'--zk'],folder,'proof')
        if int(time.time())>=a['closes_at_utc']: raise ValueError('Deadline passed during proof generation; no receipt issued')
        try:
            result=self.call([HELPER,'admit',a['path'],folder,i],folder,'acceptance',parse=True)
        except Exception:
            a['poisoned']=True # A partial quorum is never silently overwritten/retried.
            raise
        if (not result['accepted'] or not result['client_input_proof_verified'] or result['share_pairs_verified']!=3
                or result['receipt_signatures_verified']!=3 or result['chain_funding_verified']
                or result['native_settlement_started'] or result['ticket']!=ticket.hex()):
            a['poisoned']=True; raise RuntimeError('Incomplete rehearsal acceptance')
        a['receipts'].append(result)
        a['clients'].append(folder)
        opening_token=secrets.token_urlsafe(32)
        a['opening_tokens'][ticket.hex()]=opening_token
        return {**result,'opening_token':opening_token,'auction':self.public(a),
                'ticket_number':i+1,'seconds':time.monotonic()-started}

    def close(self,value):
        if type(value) is not dict or set(value)!={'auction_id'}: raise ValueError('Expected auction ID')
        a=self.lookup(value['auction_id'])
        if a['certificate']: return a['certificate']
        if a['poisoned'] or len(a['receipts'])!=4: raise ValueError('Exactly four complete signed acceptances required')
        if int(time.time())<a['closes_at_utc']: raise ValueError('Scheduled close time has not been reached')
        try: result=self.call([HELPER,'close',a['path']],a['path'],'close',parse=True)
        except Exception:
            a['poisoned']=True; raise
        a['certificate']=result
        return result

    def worker(self,action,a,index=None):
        folder=a['path']/('result-'+secrets.token_hex(8))
        args=[sys.executable,str(ROOT/'scripts/rehearsal_clearing.py'),action,str(a['path']),str(folder)]
        if a['clearing_run'] is not None: args+=['--run',str(a['clearing_run'])]
        if index is not None: args+=['--index',str(index)]
        result=guarded_json_call(args,cwd=ROOT,env={**os.environ,'RAYON_NUM_THREADS':'2'},timeout=600,
            observations=a['path']/(folder.name+'-resources.jsonl'),interval=5)
        return result,folder

    def clear(self,value):
        if type(value) is not dict or set(value)!={'auction_id'}: raise ValueError('Expected auction ID')
        a=self.lookup(value['auction_id'])
        if a['clearing'] is not None: return a['clearing']
        if not self.clearing_available: raise ValueError('Pinned MPC tools required')
        if a['clearing_failed']: raise ValueError('Incomplete clearing retained; start a new demo auction')
        if a['poisoned'] or not a['certificate'] or len(a['clients'])!=4:
            raise ValueError('Four complete acceptances and a certified closed book required')
        try:
            self.call([HELPER,'prepare-clearing',a['path']],a['path'],'prepare-clearing',
                      json.dumps(list(map(str,a['clients']))).encode(),parse=True)
            result,folder=self.worker('clear',a)
            if (not result['fixed_cpp_proof_verified'] or not result['rep3_clearing_verified']
                    or not result['certified_book_matches'] or result['certificate_signatures_verified']!=3
                    or result['accepted_root']!=a['certificate']['accepted_root'] or result['native_settlement']):
                raise RuntimeError('Fresh clearing did not match the certified book')
            a['clearing_run']=pathlib.Path(json.loads(private_read(folder/'delivery-run.json'))['run'])
            a['clearing']={**result,'auction_id':a['id']}
            return a['clearing']
        except Exception:
            a['clearing_failed']=True
            raise

    def result(self,value):
        if (type(value) is not dict or set(value)!={'auction_id','ticket','opening_token','action'}
                or value['action'] not in ('open','tamper','native-open','native-tamper')):
            raise ValueError('Expected an auction, own ticket, local opening capability and action')
        a=self.lookup(value['auction_id'])
        token=a['opening_tokens'].get(value['ticket']) if type(value['ticket']) is str else None
        if (type(value['opening_token']) is not str or not re.fullmatch(r'[A-Za-z0-9_-]{43}',value['opening_token']) or token is None
                or not hmac.compare_digest(token,value['opening_token'])):
            raise PermissionError('Own ticket opening capability required')
        if a['clearing'] is None: raise ValueError('Clear this certified book first')
        tickets=[bytes(entry['ticket']).hex() for entry in a['certificate']['certificate']['endorsements'][0]['book']['entries']]
        if value['action'].startswith('native-'):
            return self.native_receipt(a,tickets.index(value['ticket']),value['action']=='native-tamper',value['ticket'])
        result,_=self.worker(value['action'],a,tickets.index(value['ticket']))
        if value['action']=='open' and (result['accepted_root']!=a['certificate']['accepted_root']
                or not result['fixed_cpp_proof_verified'] or not result['certified_book_matches']):
            raise RuntimeError('Own result did not match the certified clearing')
        return {**result,'ticket':value['ticket'],'auction_id':a['id']}

    def native_receipt(self,a,index,tamper,ticket):
        """Open (or attack) this ticket's encrypted memo in the settled transaction."""
        from funded_public_bundle import snapshot as funded_snapshot, export as funded_export
        from open_funded_ticket import open_receipt
        from replay_nu7_auction import write_existing
        job=a['settlement']
        if job is None or job['state']!='done': raise ValueError('Settle this auction on regtest first')
        paths=job['paths']; bundle=pathlib.Path(paths['bundle']); nonce=secrets.token_hex(6)
        if tamper:
            frozen=funded_snapshot(bundle,paths['authority'],'fresh-tamper-input')
            path=frozen/'receipt-proof-public/proof.0'; data=private_read(path)
            write_existing(path,bytes([data[0]^1])+data[1:])
            bundle=ROOT/'.local'/('fresh-forged-'+nonce); funded_export(frozen,bundle)
        name='fresh-own-'+nonce
        try:
            result=open_receipt(a['path']/'clearing-book/book.json',bundle,paths['authority'],
                pathlib.Path(paths['run'])/f'private-funded-recipient-{index}.json',index,name)
        except RuntimeError as error:
            if tamper and str(error)=='Fixed C++ ticket proof rejected before native decryption':
                if any((ROOT/'.local'/name).glob('private-funded-recipient-*.json')):
                    raise RuntimeError('Tamper gate unexpectedly accessed recipient identity')
                return {'rejected':True,'key_read':False,'memo_decrypted':False,'ticket':ticket,'auction_id':a['id']}
            raise
        if tamper: raise RuntimeError('Changed receipt proof unexpectedly accepted')
        if result['txid']!=job['public']['txid'] or result['price']!=job['public']['price'] or result['sold']!=job['public']['sold']:
            raise RuntimeError('Receipt belongs to a different settlement')
        public={k:result[k] for k in ('won','refund','price','sold','txid','seconds',
            'fixed_cpp_ticket_and_auction_proofs_verified','accepted_book_and_recipients_authenticated',
            'own_ticket_opening_verified','native_amount_matches_refund','native_proof_and_signatures_verified',
            'own_viewing_key_only','receipt_in_same_six_output_transaction')}
        return {**public,'ticket':ticket,'auction_id':a['id'],'native_settlement':True}

    def start_settlement(self,value):
        """Start the long same-book settlement. Returns (view, started); a started
        job owns the proof lock until the driver ends, so no other proof overlaps."""
        if type(value) is not dict or set(value)!={'auction_id'}: raise ValueError('Expected auction ID')
        a=self.lookup(value['auction_id'])
        if a['settlement'] is not None: return self.settlement_view(a),False
        if not self.settlement_available: raise ValueError('Regtest settlement tools are unavailable on this checkout')
        if not standard_terms(a): raise ValueError('Regtest settlement supports 3 units, budget 100 and reserve 0 or 20')
        if a['clearing'] is None: raise ValueError('Clear and verify this auction first')
        import shutil
        from settle_fresh_book import MIN_START_DISK
        if shutil.disk_usage(ROOT).free < MIN_START_DISK:
            raise ValueError('Free at least 10 GiB of disk before settling; the proof guard would refuse later')
        from regtest_nu7_node import status
        try: status()
        except Exception: raise ValueError('Start the local NU7 regtest node first: python3 scripts/regtest_nu7_node.py start')
        name='settle-'+secrets.token_hex(5)
        job={'state':'running','name':name,'progress':a['path']/(name+'-progress.jsonl'),
            'started_at_unix':int(time.time()),'public':None,'paths':None}
        def work():
            try:
                with (a['path']/(name+'.log')).open('xb') as log:
                    r=subprocess.run([sys.executable,'-B',str(SETTLE),str(a['path']),'--name',name,
                        '--progress',str(job['progress']),'--expect',f"{a['clearing']['price']},{a['clearing']['sold']}"],
                        cwd=ROOT,stdout=subprocess.DEVNULL,stderr=log,timeout=7200)
                if r.returncode: raise RuntimeError('Settlement driver stopped')
                settled=json.loads(private_read(ROOT/'.local'/(name+'-settled.json'),65536))
                if (settled['public']['accepted_root']!=a['certificate']['accepted_root']
                        or (settled['public']['price'],settled['public']['sold'])!=(a['clearing']['price'],a['clearing']['sold'])):
                    raise RuntimeError('Settled a different book or outcome')
                job['paths']=settled['paths']; job['public']=settled['public']; job['state']='done'
            except Exception:
                job['state']='failed'
            finally: self.lock.release()
        a['settlement']=job
        try: threading.Thread(target=work,name='darkcall-settlement',daemon=True).start()
        except BaseException:
            a['settlement']=None; raise
        return self.settlement_view(a),True

    def post(self,action,value):
        if not self.enabled: return 409,{'error':'Local input prover/admission helper required. Recorded verification remains available.'}
        if not self.lock.acquire(blocking=False): return 409,{'error':'Another local proof, verification or settlement is running'}
        if action=='settle':
            started=False
            try:
                view,started=self.start_settlement(value)
                return 200,view
            finally:
                if not started: self.lock.release()
        try:
            try: result={'create':self.create,'bid':self.bid,'close':self.close,'clear':self.clear,'result':self.result}[action](value)
            except PermissionError: return 403,{'error':'Own ticket opening capability required'}
            return 200,result
        finally: self.lock.release()
