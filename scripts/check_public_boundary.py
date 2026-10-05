#!/usr/bin/env python3
"""Concrete portable-bundle path/key/corruption refusals on actual public proofs."""
import hashlib
import json
import os
import pathlib
import secrets
import shutil
from public_bundle import snapshot, verify_flags
from regtest_deposits import ROOT, private_write


def main():
    os.umask(0o077)
    source=ROOT/'demo/artifacts/canonical-nu7-v1'
    target=ROOT/'.local'/('portable-attacks-'+secrets.token_hex(5))
    target.mkdir(mode=0o700)
    rejected=[]
    for attack in ('missing-proof','digest','external-mpc-path','uploaded-vk','symlink','cross-run-receipt'):
        case=target/attack
        shutil.copytree(source,case)
        manifest=json.loads((case/'bundle.json').read_bytes())
        changed=None
        if attack=='missing-proof': (case/'native-binding-proof-0').unlink()
        elif attack=='digest': (case/'native-binding-proof-0').write_bytes(b'invalid')
        elif attack=='external-mpc-path':
            changed='mpc-result.json'
            (case/changed).write_text(json.dumps({'run':'../../secret-wallet','hash_rule':manifest['hash_rule']}))
        elif attack=='uploaded-vk':
            changed='receipt-proof-public/vk'
            data=(case/changed).read_bytes()
            (case/changed).write_bytes(bytes([data[0]^1])+data[1:])
        elif attack=='symlink':
            (case/'receipt-book.json').unlink()
            (case/'receipt-book.json').symlink_to(source/'receipt-book.json')
        else:
            changed='receipt-proof-public/public-input0'
            (case/changed).write_bytes((ROOT/'.local/zero-receipt-memo-v2-20261005/proof-public/public-input0').read_bytes())
        if changed:
            # Recompute the producer checksum: correctness must come from the
            # fixed verifier/context, not a self-authored packaging manifest.
            manifest['artifacts'][changed]=hashlib.sha256((case/changed).read_bytes()).hexdigest()
            (case/'bundle.json').write_text(json.dumps(manifest))
        try:
            frozen=snapshot(case,'boundary')
            if attack=='uploaded-vk': verify_flags(frozen/'receipt-proof-public','bad-vk')
        except (RuntimeError,FileNotFoundError,OSError) as error:
            wanted={'missing-proof':'No such file','digest':'digest mismatch',
                'external-mpc-path':'must not supply external MPC paths',
                'uploaded-vk':'Fixed C++ ticket proof rejected',
                'symlink':'Too many levels of symbolic links',
                'cross-run-receipt':'auction context/price/sold differ'}[attack]
            if wanted not in str(error): raise RuntimeError('Different portable rejection: '+attack) from error
            rejected.append(attack)
        else: raise RuntimeError('Portable attack accepted: '+attack)
    result={'scope':'actual public bundle input boundary; no native row proving or private inputs',
        'rejected':rejected,'producer_checksums_are_not_authentication':True}
    private_write(target/'summary.json',result)
    print(json.dumps(result,indent=2))


if __name__=='__main__': main()
