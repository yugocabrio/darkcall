#!/usr/bin/env python3
"""Allowlisted certified NU7 public replay under an explicit separate authority.

Contains no keys, PCZT, witnesses, receiver openings or private receipt pads.
An admission certificate is an operator attestation, not a public funding proof.
Native consensus validity and chain inclusion are distinct checks.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets

from public_bundle import (SDK, SDK_PIN, HASH_RULE, LIMITS as SYNTHETIC_LIMITS,
    read_public, attack_statements, link_receipt_statements, verify_flags)
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

KIND = 'darkcall-nu7-six-output-memos-certified-v1'
LIMITS = {k:v for k,v in SYNTHETIC_LIMITS.items() if k != 'synthetic-nu7-transaction.bin'}
LIMITS.update({'funded-nu7-transaction.bin':100000, 'final-book-certificate.json':131072})


def snapshot(source, authority, prefix='certified-public'):
    source = pathlib.Path(source).absolute()
    authority = pathlib.Path(authority).absolute()
    if authority.resolve(strict=True).is_relative_to(source.resolve(strict=True)):
        raise RuntimeError('Admission authority must be supplied separately from the bundle')
    manifest = json.loads(read_public(source, 'bundle.json', 16384))
    if (set(manifest) != {'format','kind','sdk','hash_rule','artifacts'}
            or manifest['format'] != 1 or manifest['kind'] != KIND
            or manifest['sdk'] != SDK_PIN or manifest['hash_rule'] != HASH_RULE
            or not isinstance(manifest['artifacts'],dict)
            or set(manifest['artifacts']) != set(LIMITS)):
        raise RuntimeError('Unknown or incomplete certified public bundle format')
    local = ROOT / '.local'
    if local.is_symlink(): raise RuntimeError('Ignored snapshot directory symlink refused')
    target = local / (prefix + '-' + secrets.token_hex(6))
    target.mkdir(mode=0o700)
    for folder in ('mpc-public','receipt-proof-public'): (target / folder).mkdir(mode=0o700)
    for name, limit in LIMITS.items():
        data = read_public(source, name, limit)
        if hashlib.sha256(data).hexdigest() != manifest['artifacts'][name]:
            raise RuntimeError('Certified public artifact digest mismatch: ' + name)
        write_bytes(target / name, data)
    # Authority is an explicit caller input; never accept a key/manifest chosen
    # by the bundle producer as the verifier's root of trust.
    write_bytes(target / 'trusted-authority.json', read_public(authority.parent, authority.name, 16384))
    result = json.loads(private_read(target / 'mpc-result.json',4096))
    if result != {'run':'mpc-public','hash_rule':HASH_RULE}:
        raise RuntimeError('Certified bundle must not supply external MPC paths')
    fd = os.open(target / 'mpc-result.json', os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
    with os.fdopen(fd,'w') as f:
        json.dump({'run':str(target / 'mpc-public'),'hash_rule':HASH_RULE},f)
        f.flush(); os.fsync(f.fileno())
    for folder,circuit in [('mpc-public','settlement_poseidon4'),('receipt-proof-public','delivery_flags4')]:
        attack_statements(target / folder,circuit)
    link_receipt_statements(private_read(target / 'receipt-proof-public/public-input0'),
                            private_read(target / 'mpc-public/public-input0'))
    return target


def export(source, target):
    source = pathlib.Path(source).resolve(strict=True)
    local = (ROOT / '.local').resolve()
    if not source.is_relative_to(local): raise RuntimeError('Export requires an ignored actual funded run')
    target = pathlib.Path(target).absolute()
    target.mkdir(mode=0o700)
    for folder in ('mpc-public','receipt-proof-public'): (target / folder).mkdir(mode=0o700)
    mpc = pathlib.Path(json.loads(private_read(source / 'mpc-result.json',4096))['run']).resolve(strict=True)
    if not mpc.is_relative_to(local): raise RuntimeError('Unexpected producer MPC path')
    hashes = {}
    for name,limit in LIMITS.items():
        if name == 'mpc-result.json':
            data = json.dumps({'run':'mpc-public','hash_rule':HASH_RULE}).encode()
        else:
            path = mpc / name.split('/',1)[1] if name.startswith('mpc-public/') else source / name
            data = private_read(path,limit)
        write_bytes(target / name,data)
        hashes[name] = hashlib.sha256(data).hexdigest()
    private_write(target / 'bundle.json',{'format':1,'kind':KIND,'sdk':SDK_PIN,
        'hash_rule':HASH_RULE,'artifacts':hashes})
    return {'public_bundle':str(target),'files':len(hashes),
        'private_keys_pads_pczt_witnesses_exported':False,'authority_included':False}


def verify(source, authority):
    target = snapshot(source,authority)
    verify_flags(target / 'receipt-proof-public','certified-flags')
    result = guarded_json_call([str(SDK),'verify-funded-auction',str(target),str(target / 'trusted-authority.json')],
        cwd=ROOT,env=ENV,timeout=900,observations=target / 'verification-resources.jsonl')
    if (not result['native_proof_and_signatures_verified'] or not result['actual_transaction_outputs_match']
            or not result['application']['native_row_proofs_verified']
            or not result['application']['normal_cpp_auction_proof_verified']
            or not result['application']['accepted_book_and_recipients_authenticated']
            or result['private_openings_keys_pczt_read'] or result['chain_funding_or_inclusion_verified']):
        raise RuntimeError('Incomplete certified public verification')
    result.update(receipt_fixed_proof_verified=True,receipt_settlement_context_link_verified=True,
        accepted_book_certified=True,private_material_required=False,
        admission_scope='operator signatures under separate caller-trusted authority; not public funding ZK')
    private_write(target / 'verification-result.json',result)
    return result


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source',type=pathlib.Path)
    parser.add_argument('--export',type=pathlib.Path)
    parser.add_argument('--authority',type=pathlib.Path)
    args = parser.parse_args()
    if args.export is None and args.authority is None:
        parser.error('Verification requires an explicit separate --authority')
    print(json.dumps(export(args.source,args.export) if args.export else verify(args.source,args.authority),indent=2))


if __name__ == '__main__': main()
