#!/usr/bin/env python3
"""Guarded cold key + repeated fresh proofs with ONE persistent shared18 PK.

This benchmarks native binding only, not an auction or signing approval. The
Mac runs concurrency1 only. 2/3/6 require measured sufficient Linux RAM; no
override. Source private openings and all output proofs remain ignored.
"""
import argparse
import json
import os
import selectors
import secrets
import subprocess
import time
from mpc_process import GROUP_ENV
from regtest_deposits import ROOT, ENV, NATIVE, private_write
from regtest_resources import sample, check, stop


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source')
    parser.add_argument('--workers',type=int,choices=[1,2,3,6],default=1)
    parser.add_argument('--rows',type=int,nargs='+',default=[0])
    parser.add_argument('--rounds',type=int,default=2)
    args=parser.parse_args()
    if not 2 <= args.rounds <= 4 or len(set(args.rows)) != len(args.rows) or any(i not in range(6) for i in args.rows):
        raise RuntimeError('Expected2–4 rounds and distinct row indices0–5')
    initial=sample(ROOT)
    check(initial)
    if initial['free_disk_bytes'] < 10*1024**3:
        raise RuntimeError('Warm benchmark not launched: retain10GiB initial proof headroom')
    name='warm-rows-'+secrets.token_hex(5)
    observations=ROOT/'.local'/(name+'-resources.jsonl')
    log=ROOT/'.local'/(name+'-stderr.log')
    started=time.monotonic()
    with observations.open('x') as metrics, log.open('xb') as errors:
        launch=['python3','-c','import os,sys; os.environ[sys.argv[1]]=str(os.getpgrp()); os.execvpe(sys.argv[2],sys.argv[2:],os.environ)',
                GROUP_ENV,str(NATIVE),'--warm-row-worker',str(args.workers)]
        child=subprocess.Popen(launch,cwd=ROOT,env=ENV,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
            stderr=errors,start_new_session=True)
        monitor=selectors.DefaultSelector()
        monitor.register(child.stdout,selectors.EVENT_READ)
        pending=b''
        messages=[]
        last_sample=0
        try:
            while len(messages)<args.rounds+1:
                now=time.monotonic()
                if now-started>3600: raise RuntimeError('Warm benchmark timed out; artifacts retained')
                if now-last_sample>=10:
                    observed=sample(ROOT,child.pid)
                    metrics.write(json.dumps({'seconds':now-started,**observed})+'\n')
                    metrics.flush(); os.fsync(metrics.fileno())
                    check(observed); last_sample=now
                for key,_ in monitor.select(timeout=1):
                    data=os.read(key.fileobj.fileno(),4097)
                    if not data: raise RuntimeError('Warm worker stopped; inspect retained diagnostic')
                    pending+=data
                    if len(pending)>65536: raise RuntimeError('Warm worker output bound')
                    while b'\n' in pending:
                        line,pending=pending.split(b'\n',1)
                        value=json.loads(line)
                        if not messages:
                            if value.get('ready') is not True: raise RuntimeError('Warm worker not ready')
                            for round_index in range(args.rounds):
                                job={'source':args.source,'name':name+'-'+str(round_index),'rows':args.rows}
                                child.stdin.write(json.dumps(job).encode()+b'\n')
                            child.stdin.flush(); child.stdin.close()
                        elif value.get('completed') is not True or value.get('proving_key_generated') is not False:
                            raise RuntimeError('Warm worker did not reuse its fixed key')
                        messages.append(value)
            if child.wait(timeout=10)!=0: raise RuntimeError('Warm worker failed after proof results')
            result={'scope':'same fixed shared18 native row proof benchmark; not complete auction latency',
                'workers':args.workers,'rounds':args.rounds,'messages':messages,
                'measured_seconds':time.monotonic()-started,'key_generations':1,
                'native_signatures_or_auction_approval_created':False}
            private_write(ROOT/'.local'/(name+'-summary.json'),result)
            print(json.dumps(result,indent=2))
        finally:
            monitor.close()
            stop(child)


if __name__=='__main__': main()
