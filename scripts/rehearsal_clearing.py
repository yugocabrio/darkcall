#!/usr/bin/env python3
"""Existing REP3 flags circuit for one newly certified offline browser book.

No plaintext bid openings, native signing, funding RPC or recorded-run reuse.
Run under the existing owned-process resource guard; stdout is public JSON only.
"""
import argparse
import json
import os
import pathlib
import secrets
import subprocess
import sys
import time
from regtest_deposits import ROOT, private_read
from regtest_prepared_gate import write_bytes

HELPER=ROOT/'target/release/examples/rehearsal_admission'
INPUT=ROOT/'target/release/darkcall-input-tool'


def command(args,folder,label,parse=False):
    with (folder/(label+'.log')).open('xb') as log:
        r=subprocess.run(list(map(str,args)),cwd=ROOT,stdout=subprocess.PIPE if parse else log,
                         stderr=log,timeout=120)
    if r.returncode: raise RuntimeError('Fresh clearing gate stopped; private checkpoint retained')
    return json.loads(r.stdout) if parse else None


def verify(auction,run,folder):
    command([sys.executable,ROOT/'scripts/verify_external.py',run,'--circuit','delivery_flags4',
             '--output-label','rehearsal-'+secrets.token_hex(6)],folder,'fixed-proof')
    return command([HELPER,'check-clearing',auction,run],folder,'certified-context',parse=True)


def execute(action,auction,folder,run=None,index=None):
    started=time.monotonic();folder.mkdir(mode=0o700)
    if action=='clear':
        link=folder/'delivery-run.json'
        command([sys.executable,ROOT/'scripts/run_delivery_gate.py','--book-run',auction/'clearing-book',
                 '--winner-flags','--result-file',link],folder,'rep3')
        run=pathlib.Path(json.loads(private_read(link))['run']).resolve(strict=True)
    if run is None or not run.resolve(strict=True).is_relative_to((ROOT/'.local').resolve()):
        raise RuntimeError('Owned ignored MPC checkpoint required')
    if action=='tamper':
        forged=folder/'forged';forged.mkdir(mode=0o700)
        for name in ['vk','proof.0','public-input0','tampered-price','tampered-root','tampered-cipher','tampered-winner']:
            data=private_read(run/name,1048576)
            if name=='proof.0': data=bytes([data[0]^1])+data[1:]
            write_bytes(forged/name,data)
        with (folder/'attempt.log').open('xb') as log:
            r=subprocess.run([sys.executable,str(ROOT/'scripts/verify_external.py'),str(forged),
                              '--circuit','delivery_flags4','--output-label','tamper'],cwd=ROOT,
                             stdout=log,stderr=log,timeout=120)
        if r.returncode==0 or b'Proof verification failed' not in private_read(forged/'tamper/valid-proof.log',1048576):
            raise RuntimeError('Expected cryptographic refusal absent')
        return {'rejected':True,'private_pad_read':False,'native_settlement':False}
    linked=verify(auction,run,folder)
    if action=='open':
        book=auction/'clearing-book/book.json';receipt=folder/'private-own-ticket.json'
        command([INPUT,'flags-ticket-export',book,run/'plan/masks.json',run/'plan/winner-masks.json',
                 run/'public-input0',index,receipt],folder,'own-export')
        opened=command([INPUT,'flags-ticket-open',book,index,receipt,run/'public-input0'],folder,'own-open',parse=True)
        return {**opened,'fixed_cpp_proof_verified':True,'certified_book_matches':True,
                'accepted_root':linked['accepted_root'],'native_settlement':False,'funding_scope':'offline-synthetic'}
    summary=json.loads(private_read(run/'summary.json',32768))
    if (not summary['passed'] or not summary['mask_commitments_blinded']
            or not summary['masked_winner_flags_delivered'] or summary['legacy_candidate_guessing_leak']
            or summary['price']!=linked['price'] or summary['sold']!=linked['sold']):
        raise RuntimeError('Unexpected clearing proof scope')
    # Output only public fields. Operator refunds/winner flags and pad openings
    # remain private. The parent reads delivery-run.json internally, not over HTTP.
    return {**linked,'fixed_cpp_proof_verified':True,'rep3_clearing_verified':True,
        'proof_generation_seconds':next(s['seconds'] for s in summary['stages'] if s['name']=='proof'),
        'seconds':time.monotonic()-started,'maximum_bid_openings_read':False,
        'receipt_proof_version':'delivery_flags4/v2','proof_bytes':summary['proof_bytes']}


def main():
    os.umask(0o077)
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['clear','open','tamper'])
    p.add_argument('auction',type=pathlib.Path);p.add_argument('folder',type=pathlib.Path)
    p.add_argument('--run',type=pathlib.Path);p.add_argument('--index',type=int,choices=range(4))
    a=p.parse_args();local=(ROOT/'.local').resolve()
    if any(not v.resolve().is_relative_to(local) for v in (a.auction,a.folder)):
        raise RuntimeError('Ignored owned rehearsal paths only')
    if a.action=='open' and a.index is None: raise RuntimeError('Own ticket index required')
    print(json.dumps(execute(a.action,a.auction,a.folder,a.run,a.index)))


if __name__=='__main__': main()
