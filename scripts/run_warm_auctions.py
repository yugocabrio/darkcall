#!/usr/bin/env python3
"""Warm ONE fixed PK, then run bounded complete synthetic NU7 memo auctions.

The row-only service remains separate. This service calls the existing native
finalizer, which independently verifies the same application before signing.
No chain funds/RPC/broadcast, imported keys, or weaker verifier are supported.
"""
import argparse
import json
import os
import pathlib
import secrets
import selectors
import subprocess
import time
from mpc_process import GROUP_ENV
from regtest_deposits import ROOT, ENV, NATIVE, private_write
from regtest_resources import sample, check, stop


def observed_group(pid):
    value = sample(ROOT, pid)
    measured = subprocess.run(['ps', '-axo', 'pgid=,rss='],
        capture_output=True, text=True, timeout=5, check=True)
    resident = 0
    for line in measured.stdout.splitlines():
        fields = line.split()
        if len(fields) == 2 and int(fields[0]) == pid:
            resident += int(fields[1])
    # Include SDK/MPC descendants while the parent's PK remains resident. Apply
    # the existing 8GiB limit to the entire owned group; never relax Mac guards.
    value['owned_group_resident_kib'] = resident
    value['prover_resident_kib'] = max(value.get('prover_resident_kib', 0), resident)
    return value


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workers', type=int, choices=[1, 2, 3, 6], default=1)
    parser.add_argument('--job', nargs=2, action='append', required=True,
                        metavar=('BOOK_RUN', 'FRESH_NAME'))
    args = parser.parse_args()
    if not 1 <= len(args.job) <= 4:
        raise RuntimeError('Expected one to four complete synthetic auction jobs')
    jobs = []
    for path, name in args.job:
        book = pathlib.Path(path).resolve(strict=True)
        if (not book.is_relative_to((ROOT/'.local').resolve()) or not book.is_dir()
                or not name or len(name) > 50
                or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name)
                or (ROOT/'.local'/name).exists() or any(j['name'] == name for j in jobs)):
            raise RuntimeError('Expected ignored book and fresh distinct simple auction name')
        jobs.append({'book':str(book), 'name':name})
    initial = sample(ROOT)
    check(initial)
    if initial['free_disk_bytes'] < 10*1024**3:
        raise RuntimeError('Warm auction not launched: retain10GiB initial proof headroom')
    name = 'warm-auctions-' + secrets.token_hex(5)
    started = time.monotonic()
    with (ROOT/'.local'/(name+'-resources.jsonl')).open('x') as metrics, \
            (ROOT/'.local'/(name+'-stderr.log')).open('xb') as errors:
        launch = ['python3', '-c',
            'import os,sys; os.environ[sys.argv[1]]=str(os.getpgrp()); os.execvpe(sys.argv[2],sys.argv[2:],os.environ)',
            GROUP_ENV, str(NATIVE), '--nu7-warm-auction-worker', str(args.workers)]
        child = subprocess.Popen(launch, cwd=ROOT, env=ENV, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=errors, start_new_session=True)
        monitor = selectors.DefaultSelector()
        monitor.register(child.stdout, selectors.EVENT_READ)
        messages, pending, stages = [], b'', []
        last_sample, stage_start = 0, started
        try:
            while len(messages) < len(jobs)+1:
                now = time.monotonic()
                if now-stage_start > 3600:
                    raise RuntimeError('Warm auction stage timed out; artifacts retained')
                if now-last_sample >= 10:
                    observed = observed_group(child.pid)
                    metrics.write(json.dumps({'seconds':now-started, **observed})+'\n')
                    metrics.flush(); os.fsync(metrics.fileno())
                    check(observed); last_sample = now
                for key, _ in monitor.select(timeout=1):
                    data = os.read(key.fileobj.fileno(), 4097)
                    if not data:
                        raise RuntimeError('Warm auction worker stopped; inspect retained diagnostic')
                    pending += data
                    if len(pending) > 65536:
                        raise RuntimeError('Warm auction worker output bound')
                    while b'\n' in pending:
                        line, pending = pending.split(b'\n', 1)
                        value = json.loads(line)
                        if value.get('stage') == 'clearing_verified':
                            if (not messages or len(messages) > len(jobs)
                                    or value.get('job') != jobs[len(messages)-1]['name']
                                    or value.get('fixed_cpp_delivery_proof_verified') is not True
                                    or value.get('six_output_settlement_verified') is not False
                                    or value.get('chain_funding') is not False
                                    or any(s['job'] == value['job'] for s in stages)):
                                raise RuntimeError('Unexpected clearing stage or stronger settlement claim')
                            value['seconds_since_job_start'] = time.monotonic()-stage_start
                            stages.append(value)
                            print(json.dumps(value), flush=True)
                            continue
                        if not messages:
                            if value.get('ready') is not True or value.get('workers') != args.workers:
                                raise RuntimeError('Fixed auction worker not ready')
                        else:
                            expected = jobs[len(messages)-1]['name']
                            result = value.get('result', {})
                            native = result.get('native', {})
                            if (value.get('completed') is not True or value.get('job') != expected
                                    or value.get('proving_key_generated') is not False
                                    or result.get('passed') is not True
                                    or result.get('warm_proving_key_reused') is not True
                                    or result.get('chain_funding') is not False
                                    or native.get('four_receipt_memos_in_same_six_output_transaction') is not True):
                                raise RuntimeError('Incomplete or mismatched warm auction result')
                            clearing = next((s for s in stages if s['job'] == expected), None)
                            if (clearing is None or clearing['price'] != result['price']
                                    or clearing['sold'] != result['sold']):
                                raise RuntimeError('Final settlement differs from verified clearing stage')
                            private_write(ROOT/'.local'/(expected+'-summary.json'),
                                {'scope':'synthetic warm NU7 integration; no chain funding', 'result':result})
                        messages.append(value)
                        print(json.dumps(value), flush=True)
                        stage_start = time.monotonic()
                        if len(messages) <= len(jobs):
                            # Send each job only after ready/previous completion.
                            child.stdin.write(json.dumps(jobs[len(messages)-1]).encode()+b'\n')
                            child.stdin.flush()
                        else:
                            child.stdin.close()
            if child.wait(timeout=10) != 0:
                raise RuntimeError('Warm auction worker failed after its results')
            result = {'scope':'complete synthetic NU7 auctions sharing one fixed PK; no chain funding',
                'workers':args.workers, 'key_generations':1, 'messages':messages, 'stages':stages,
                'seconds':time.monotonic()-started,
                'resource_guard':'existing8GiB ceiling applied to the entire owned process group'}
            private_write(ROOT/'.local'/(name+'-summary.json'), result)
            print(json.dumps({'summary':name, 'seconds':result['seconds'], 'key_generations':1}), flush=True)
        finally:
            monitor.close()
            stop(child)


if __name__ == '__main__':
    main()
