"""Fresh LOCAL seller/bidder rehearsal, disjoint from recorded settlement.

The localhost client helper sees typed demo bids. It generates the existing
Noir ZK input proof and REP3 shares, then checks the proof/pairs before issuing
three real signed receipts for explicitly synthetic funding. Never real bids,
RPC, canonical wallet/funding, native signing or broadcast. Restart starts a
new rehearsal namespace; ignored artifacts are retained, never auto-resumed.
"""
import hashlib
import json
import os
import pathlib
import re
import secrets
import subprocess
import time
from regtest_deposits import ROOT
from regtest_resources import check, sample
from run_mpc_gate import INPUT_TOOL, PLAIN, CLIENT_CIRCUIT, UPSTREAM, PIN
from verify_external import BB, SOURCE, CRS, PIN as BB_PIN

HELPER=ROOT/'target/release/examples/rehearsal_admission'
ID=re.compile(r'[0-9a-f]{32}\Z')


def number(value,minimum,maximum):
    if type(value) is not int or not minimum<=value<=maximum:
        raise ValueError('Use bounded whole-number demo amounts and counts')
    return value


class Rehearsal:
    def __init__(self,lock,enabled=True):
        self.lock=lock
        self.enabled=enabled and all(p.is_file() for p in (HELPER,INPUT_TOOL,PLAIN,CLIENT_CIRCUIT,BB))
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
        if subprocess.check_output(['git','-C',str(SOURCE),'diff','--'])!=patches:
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
            'chain_funding_verified':False,'fresh_clearing_or_settlement':False}

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
            'digest':list(digest),'path':path,'authority':initialized['authority'],'receipts':[],'certificate':None,'poisoned':False}
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
        return {**result,'auction':self.public(a),'ticket_number':i+1,'seconds':time.monotonic()-started}

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

    def post(self,action,value):
        if not self.enabled: return 409,{'error':'Local input prover/admission helper required. Recorded verification remains available.'}
        if not self.lock.acquire(blocking=False): return 409,{'error':'Another local proof or verification is running'}
        try:
            result={'create':self.create,'bid':self.bid,'close':self.close}[action](value)
            return 200,result
        finally: self.lock.release()
