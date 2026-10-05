#!/usr/bin/env python3
"""Fast funded public replay substitutions before expensive row verification.

Uses a private public-only snapshot; original run, authority and journals are
untouched. The positive complete replay is a separate measured command.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
from funded_public_bundle import snapshot
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_nu7_funding import SDK
from replay_nu7_auction import write_existing


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle',type=pathlib.Path)
    parser.add_argument('authority',type=pathlib.Path)
    parser.add_argument('--name',required=True)
    args=parser.parse_args()
    if (not args.name or len(args.name)>50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh ignored check name required')
    target=snapshot(args.bundle,args.authority,args.name)
    authority=target / 'trusted-authority.json'
    original_authority=hashlib.sha256(private_read(args.authority)).hexdigest()
    original_manifest=hashlib.sha256((args.bundle / 'bundle.json').read_bytes()).hexdigest()
    start=time.monotonic()
    cases={
        'trailing-byte':('funded-nu7-transaction.bin',b'trailing data'),
        'binding-signature':('funded-nu7-transaction.bin',b'native proof or signatures invalid'),
        'identity':('nu7-native-identity.json',b'funded public native effects identity mismatch'),
        'action-collision':('native-public.json',b'NU7 actual native output/action substitution'),
        'certificate-signature':('final-book-certificate.json',b'final-book operator signature invalid'),
        'accepted-root':('mpc-public/public-input0',b'MPC book/root/recipient differs from certified acceptance'),
        'authority-key':('trusted-authority.json',b'final book differs from trusted auction manifest'),
        'authority-cutoff':('trusted-authority.json',b'final book differs from trusted auction manifest'),
    }
    rejections=[]
    for label,(name,expected) in cases.items():
        path=target/name
        data=private_read(path)
        if label=='trailing-byte': changed=data+b'\0'
        elif label=='binding-signature': changed=data[:-1]+bytes([data[-1]^1])
        elif label=='accepted-root':
            changed=bytearray(data)
            changed[168*32+31]^=1
            changed=bytes(changed)
        else:
            value=json.loads(data)
            if label=='identity': value['effects_digest'][0]^=1
            elif label=='action-collision': value['action_indices'][0]=value['action_indices'][1]
            elif label=='certificate-signature':
                sig=bytearray.fromhex(value['endorsements'][0]['signature'])
                sig[0]^=1
                value['endorsements'][0]['signature']=sig.hex()
            elif label=='authority-key': value['operator_keys'][0][0]^=1
            else: value['closes_at_utc']+=1
            changed=json.dumps(value).encode()
        try:
            write_existing(path,changed)
            result=subprocess.run([str(SDK),'verify-funded-auction',str(target),str(authority)],
                cwd=ROOT,env=ENV,capture_output=True,timeout=60)
            private_write(target/(label+'-diagnostic.json'),{'stderr':result.stderr.decode('utf-8',errors='replace')})
            if result.returncode==0 or expected not in result.stderr or b'panicked at' in result.stderr:
                raise RuntimeError('Intended funded public refusal not observed: '+label)
            rejections.append(label)
        finally:
            write_existing(path,data)
    if (hashlib.sha256(private_read(args.authority)).hexdigest()!=original_authority
            or hashlib.sha256((args.bundle/'bundle.json').read_bytes()).hexdigest()!=original_manifest):
        raise RuntimeError('Original authority or bundle changed')
    summary={'scope':'funded public-only fast substitution refusals; no secret artifacts',
        'rejected':rejections,'source_unchanged':True,'seconds':time.monotonic()-start}
    private_write(ROOT/'.local'/(args.name+'-summary.json'),summary)
    print(json.dumps(summary,indent=2))


if __name__=='__main__': main()
