#!/usr/bin/env python3
"""Audited archive → fresh public-only view, on this same machine.

No dependency sharing, keys, proving, RPC or chain operations. This is an archive
and browser/API smoke check, not a second-machine or cryptographic replay claim.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets
import socket
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.request
from review_kit import ROOT, TXID, check


def smoke(archive, expected_commit):
    if sys.version_info<(3,12): raise RuntimeError('Safe extraction smoke requires Python 3.12+')
    checked=check(archive,expected_commit)
    local=ROOT/'.local'
    if local.is_symlink(): raise RuntimeError('Local output symlink refused')
    local.mkdir(mode=0o700,exist_ok=True)
    target=local/('review-kit-smoke-'+secrets.token_hex(6)); target.mkdir(mode=0o700)
    # Only extract after bounded read-only audit has refused all links/traversal.
    with tarfile.open(archive,'r:gz') as package:
        package.extractall(target,filter='data')
    source=target/('darkcall-review-'+expected_commit[:12])/'source'
    if any((source/name).exists() for name in ('.git','.local','.deps','target')):
        raise RuntimeError('Fresh source unexpectedly includes runtime/dependencies')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
    origin='http://127.0.0.1:'+str(port)
    environment={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'}
    log_path=target/'browser.log'; started=time.monotonic()
    with log_path.open('xb') as log:
        child=subprocess.Popen(['sh',str(source/'demo.sh'),'--public-only','--port',str(port)],
            cwd=source,env=environment,stdout=log,stderr=subprocess.STDOUT)
        try:
            deadline=time.monotonic()+20
            while True:
                if child.poll() is not None: raise RuntimeError('Extracted public view did not start')
                try:
                    with urllib.request.urlopen(origin+'/api/state',timeout=1) as response:
                        state=json.load(response)
                    break
                except urllib.error.URLError:
                    if time.monotonic()>deadline: raise RuntimeError('Extracted public view startup timeout')
                    time.sleep(.1)
            if (state['cases'][0]['txid']!=TXID or not state['funded_canonical']
                    or state['recipient_identities_available'] or state['chain_inclusion_verified']
                    or state['recorded_local_confirmations'] is not None):
                raise RuntimeError('Extracted public-only scope differs')
            for name in ('/','/app.js','/style.css'):
                with urllib.request.urlopen(origin+name,timeout=2) as response:
                    if response.status!=200: raise RuntimeError('Static review page unavailable')
            request=urllib.request.Request(origin+'/api/verify',
                data=json.dumps({'case':'positive','ticket':0,'action':'verify'}).encode(),
                headers={'Content-Type':'application/json','Origin':origin,'X-DarkCall-Token':state['csrf']})
            try:
                urllib.request.urlopen(request,timeout=2)
                raise RuntimeError('Public-only view unexpectedly opened an identity')
            except urllib.error.HTTPError as error:
                if error.code!=409 or 'private recipient identity' not in json.load(error)['error']:
                    raise RuntimeError('Missing identity did not refuse correctly')
            preflight=subprocess.run([sys.executable,'-B',str(source/'scripts/reproduction_check.py')],
                cwd=source,env=environment,capture_output=True,timeout=30)
            inventory=json.loads(preflight.stdout)
            if preflight.returncode!=1 or inventory['passed'] or inventory['private_runtime_read']:
                raise RuntimeError('Missing pinned tools did not report incomplete reproduction')
            if any(p.name.startswith('private-') or p.suffix in ('.key','.pczt','.db')
                   for p in (source/'.local').rglob('*')):
                raise RuntimeError('Public-only smoke created private material')
            return {'passed':True,'scope':'same-host extracted source/public-only smoke; no cryptographic replay',
                'commit':expected_commit,'archive_sha256':hashlib.sha256(pathlib.Path(archive).read_bytes()).hexdigest(),
                'source_files':checked['source_files'],'txid':TXID,'static_files_served':3,
                'own_receipt_without_identity_refused':True,'missing_tools_reported':True,
                'private_runtime_or_dependencies_copied':False,'second_machine_reproduction':False,
                'proofs_cryptographically_verified':False,'seconds':time.monotonic()-started}
        finally:
            if child.poll() is None: child.terminate()
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired: child.kill(); child.wait(timeout=5)


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive',type=pathlib.Path); parser.add_argument('--expected-commit',required=True)
    args=parser.parse_args(); print(json.dumps(smoke(args.archive,args.expected_commit),indent=2))


if __name__=='__main__': main()
