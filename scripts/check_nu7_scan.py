#!/usr/bin/env python3
"""Replay actual NU7 node ciphertexts; refuse substituted capture/cache inputs.

Private test copies stay ignored. The scanner still trusts the pinned node;
these refusals are encoding/continuity tests, not SPV/consensus verification.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time
from regtest_nu7_funding import SDK
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('wallet', type=pathlib.Path)
    parser.add_argument('capture', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if (not args.name or len(args.name) > 50
            or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name)):
        raise RuntimeError('Fresh ignored NU7 scan check name required')
    target = ROOT/'.local'/args.name
    target.mkdir(mode=0o700)
    source = json.loads(private_read(args.capture, 100_000_000))
    if not source['blocks']:
        raise RuntimeError('Actual nonempty node capture required')
    notes_name = f'private-nu7-notes-{source["final_tree"]["height"]}.json'
    original = private_read(args.wallet/notes_name)
    before = hashlib.sha256(original).hexdigest()
    started = time.monotonic()
    replay = subprocess.run([str(SDK),'regtest-scan',str(args.wallet),str(args.capture)],
        cwd=ROOT, env=ENV, capture_output=True, timeout=600, check=True)
    checked = json.loads(replay.stdout)
    if not checked['all_native_paths_match_node_tree'] or checked['funding_observations_fabricated']:
        raise RuntimeError('Actual canonical scan did not replay')
    rejected = []
    reasons = {'branch':'capture differs from pinned isolated NU7 regtest',
        'genesis':'capture differs from pinned isolated NU7 regtest',
        'block-link':'NU7 captured block continuity/order/shape mismatch',
        'tree-size':'NU7 scanned native tree size differs from node',
        'trailing-tx':'NU7 raw transaction branch/trailing data',
        'cached-amount':'NU7 persisted notes differ from fresh canonical scan'}
    for label, reason in reasons.items():
        data = json.loads(json.dumps(source))
        wallet = args.wallet
        if label == 'branch': data['branch'] = '37a5165b'
        elif label == 'genesis': data['genesis'] = '00'*32
        elif label == 'block-link': data['blocks'][0]['prev_hash'] = '00'*32
        elif label == 'tree-size': data['blocks'][0]['ironwood_size'] += 1
        elif label == 'trailing-tx': data['blocks'][0]['transactions'][0] += '00'
        else:
            wallet = target/'forged-cache-wallet'
            wallet.mkdir(mode=0o700)
            write_bytes(wallet/'private-spending-key.bin',private_read(args.wallet/'private-spending-key.bin',32))
            changed = json.loads(original)
            changed['notes'][0]['amount'] += 1
            private_write(wallet/notes_name,changed)
        path = target/(label+'-capture.json')
        private_write(path,data)
        result = subprocess.run([str(SDK),'regtest-scan',str(wallet),str(path)],
            cwd=ROOT, env=ENV, capture_output=True, timeout=600)
        write_bytes(target/(label+'-rejection.log'),result.stdout+result.stderr)
        if result.returncode == 0 or reason.encode() not in result.stderr:
            raise RuntimeError('Intended NU7 scan refusal absent: '+label)
        rejected.append(label)
    if hashlib.sha256(private_read(args.wallet/notes_name)).hexdigest() != before:
        raise RuntimeError('Canonical source notes changed during scan checks')
    summary = {'scope':'actual isolated NU7 node capture replay and six input/cache refusals',
        'canonical_replay':checked,'rejected':rejected,'original_wallet_unchanged':True,
        'private_test_copies_committed':False,'chain_consensus_verified_by_scanner':False,
        'seconds':time.monotonic()-started}
    private_write(target/'summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__ == '__main__': main()
