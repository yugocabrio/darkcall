#!/usr/bin/env python3
"""Portable, allowlisted NU7 public artifacts. Never follow a bundle's paths.

Manifest hashes detect packaging damage, not producer authenticity. The actual
fixed native/Halo2/C++ verifiers establish validity. No authority is discovered
in this bundle; funded admission is explicitly outside this synthetic run.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets
import stat
import subprocess
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

SDK_PIN = '9c5705f56517c040c50f96d29f1ff15a9383f3e0'
SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'
HASH_RULE = 'bn254-poseidon2-packed-row-v1'
LIMITS = {'synthetic-nu7-transaction.bin':100000, 'nu7-native-identity.json':16384,
    'native-public.json':32768, 'native-layout.json':4096, 'mpc-result.json':4096,
    'receipt-book.json':32768}
LIMITS.update({f'native-binding-proof-{i}':65536 for i in range(6)})
for folder in ('mpc-public','receipt-proof-public'):
    LIMITS.update({folder+'/'+name:1048576 for name in ('vk','proof.0','public-input0')})
LIMITS['mpc-public/public-input0'] = 232*32
LIMITS['receipt-proof-public/public-input0'] = 187*32


def read_public(source, name, limit):
    """Accept ordinary public file permissions, reject links/special files/oversize."""
    source = pathlib.Path(source)
    path = source / name
    for parent in [source, *path.relative_to(source).parents]:
        checked = parent if parent == source else source / parent
        if checked.is_symlink(): raise RuntimeError('Public bundle directory symlink refused')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd,'rb') as handle:
        meta = os.fstat(handle.fileno())
        if not stat.S_ISREG(meta.st_mode) or meta.st_nlink != 1 or meta.st_size > limit:
            raise RuntimeError('Public bundle artifact type/links/size differ: '+name)
        data = handle.read(limit+1)
        if len(data)>limit: raise RuntimeError('Public bundle artifact exceeds bound')
        return data


def attack_statements(directory, circuit):
    statement = private_read(directory/'public-input0', 232*32)
    positions = {'price':181,'plan':183,'row':184} if circuit == 'settlement_poseidon4' else {'price':177,'root':168,'cipher':179,'winner':183}
    for name,index in positions.items():
        changed = bytearray(statement)
        value = int.from_bytes(changed[index*32:(index+1)*32],'big')
        changed[index*32:(index+1)*32] = (value-1 if value else 1).to_bytes(32,'big')
        write_bytes(directory/('tampered-'+name),changed)


def link_receipt_statements(receipt, settlement):
    if (len(receipt) != 187*32 or len(settlement) != 232*32
            or receipt[:169*32] != settlement[:169*32]
            or receipt[177*32:179*32] != settlement[181*32:183*32]):
        raise RuntimeError('Receipt and settlement auction context/price/sold differ')


def snapshot(source, prefix='verify-public'):
    """Freeze external public inputs, bounded fixed names only, into ignored state."""
    source = pathlib.Path(source).absolute()
    manifest = json.loads(read_public(source,'bundle.json',16384))
    if (set(manifest) != {'format','kind','sdk','hash_rule','artifacts'}
            or manifest['format'] != 1 or manifest['kind'] != 'darkcall-nu7-six-output-memos-synthetic-v1'
            or manifest['sdk'] != SDK_PIN or manifest['hash_rule'] != HASH_RULE
            or not isinstance(manifest['artifacts'],dict) or set(manifest['artifacts']) != set(LIMITS)):
        raise RuntimeError('Unknown or incomplete public bundle format')
    local = ROOT/'.local'
    if local.is_symlink(): raise RuntimeError('Ignored snapshot directory symlink refused')
    local.mkdir(mode=0o700,exist_ok=True)
    target = local/(prefix+'-'+secrets.token_hex(6))
    target.mkdir(mode=0o700)
    for folder in ('mpc-public','receipt-proof-public'): (target/folder).mkdir(mode=0o700)
    for name,limit in LIMITS.items():
        data = read_public(source,name,limit)
        if hashlib.sha256(data).hexdigest() != manifest['artifacts'][name]:
            raise RuntimeError('Public artifact digest mismatch: '+name)
        write_bytes(target/name,data)
    result = json.loads(private_read(target/'mpc-result.json',4096))
    if result != {'run':'mpc-public','hash_rule':HASH_RULE}:
        raise RuntimeError('Portable bundle must not supply external MPC paths')
    # Rewrite only our frozen metadata, never an input directory or supplied path.
    path = target/'mpc-result.json'
    fd = os.open(path,os.O_WRONLY|os.O_TRUNC|os.O_NOFOLLOW)
    with os.fdopen(fd,'w') as out:
        json.dump({'run':str(target/'mpc-public'),'hash_rule':HASH_RULE},out)
        out.flush(); os.fsync(out.fileno())
    for folder,circuit in [('mpc-public','settlement_poseidon4'),('receipt-proof-public','delivery_flags4')]:
        attack_statements(target/folder,circuit)
    link_receipt_statements(private_read(target/'receipt-proof-public/public-input0'),
                            private_read(target/'mpc-public/public-input0'))
    return target


def verify_flags(proof, label):
    with (proof/(label+'.log')).open('xb') as log:
        checked = subprocess.run(['python3',str(ROOT/'scripts/verify_external.py'),str(proof),
            '--circuit','delivery_flags4','--output-label',label],cwd=ROOT,env=ENV,
            stdout=log,stderr=subprocess.STDOUT,timeout=120)
    if checked.returncode: raise RuntimeError('Fixed C++ ticket proof rejected before native decryption')


def verify(source):
    target = snapshot(source)
    verify_flags(target/'receipt-proof-public','portable-flags')
    native = guarded_json_call([str(SDK),'verify-auction',str(target)],cwd=ROOT,env=ENV,
        timeout=900,observations=target/'verification-resources.jsonl')
    if (not native['native_proof_and_signatures_verified']
            or not native['actual_transaction_outputs_match']
            or not native['application']['native_row_proofs_verified']
            or not native['application']['normal_cpp_auction_proof_verified']
            or native['private_openings_keys_pczt_read'] or native['chain_funding_or_inclusion_verified']):
        raise RuntimeError('Incomplete public-only verification')
    native.update(receipt_fixed_proof_verified=True,receipt_settlement_context_link_verified=True,
        accepted_book_certified=False,private_material_required=False)
    private_write(target/'verification-result.json',native)
    return native


def export(source, target):
    source = pathlib.Path(source).resolve(strict=True)
    target = pathlib.Path(target).absolute()
    local = (ROOT/'.local').resolve()
    if not source.is_relative_to(local): raise RuntimeError('Producer export requires ignored synthetic run')
    target.mkdir(mode=0o700)
    for folder in ('mpc-public','receipt-proof-public'): (target/folder).mkdir(mode=0o700)
    mpc = pathlib.Path(json.loads(private_read(source/'mpc-result.json',4096))['run']).resolve(strict=True)
    if not mpc.is_relative_to(local): raise RuntimeError('Unexpected producer MPC directory')
    hashes = {}
    for name,limit in LIMITS.items():
        if name == 'mpc-result.json':
            data = json.dumps({'run':'mpc-public','hash_rule':HASH_RULE}).encode()
        else:
            path = mpc/name.split('/',1)[1] if name.startswith('mpc-public/') else source/name
            data = private_read(path,limit)
        write_bytes(target/name,data)
        hashes[name] = hashlib.sha256(data).hexdigest()
    private_write(target/'bundle.json',{'format':1,'kind':'darkcall-nu7-six-output-memos-synthetic-v1',
        'sdk':SDK_PIN,'hash_rule':HASH_RULE,'artifacts':hashes})
    return {'public_bundle':str(target),'files':len(hashes),'private_keys_pads_pczt_exported':False}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source',type=pathlib.Path)
    parser.add_argument('--export',type=pathlib.Path)
    args=parser.parse_args()
    print(json.dumps(export(args.source,args.export) if args.export else verify(args.source),indent=2))


if __name__ == '__main__': main()
