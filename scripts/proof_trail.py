#!/usr/bin/env python3
"""The proof trail behind the settled page's four claims, from public files only.

Reads the committed public bundle (every file checked against the digest in its
bundle.json), the separately trusted authority, and the committed evidence for
the recorded block. It reads nothing under .local: no share, key, pad, bid or
wallet. Values are shown next to the cross-checks that tie one artifact to
another (the same root in the signed book and in the proof's public inputs, the
same row hash in the proof and in the Halo2 statement, and so on). Those
equalities are recomputed here, and the transaction's txid and note
commitments are parsed from the transaction bytes themselves (the SDK's
parse-only transaction-identity command). They are consistency checks between
files, not proofs: the proofs and signatures are checked by the verifier
(Re-run verification). Tickets are named by their place in the public book,
which is sorted by ticket id, so nothing beyond the bundle is added.

Public-input layout of the auction (settlement) statement, 232 fields, from
circuits/settlement_poseidon4/src/main.nr: condition digest 0-31, ticket ids
32-159, units 160, reserve 161, budget 162, processing fee 163, bid commitments
164-167, accepted root 168, receivers 169-174, action indices 175-180, then the
outputs price 181, sold 182, plan 183 and six row hashes 184-231.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
from funded_public_bundle import LIMITS, read_public
from public_bundle import SDK

ROOT = pathlib.Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'evidence/2026-10-19-public-testnet-settlement.json'
ROLES = ['Seller', 'Operator fee']


def fields(data, count):
    if len(data) != count * 32:
        raise RuntimeError('Unexpected public-input size')
    return [int.from_bytes(data[i:i + 32], 'big') for i in range(0, len(data), 32)]


def field_hex(value):
    return '0x' + value.to_bytes(32, 'big').hex()


def row_hex(words):
    return '0x' + b''.join(word.to_bytes(4, 'big') for word in words).hex()


def parsed_transaction(raw):
    """txid and per-action cmx, parsed from the transaction bytes by the SDK."""
    result = subprocess.run([str(SDK), 'transaction-identity'], input=raw, capture_output=True, timeout=60)
    if result.returncode:
        raise RuntimeError('Transaction bytes did not parse')
    return json.loads(result.stdout)


def recorded_block(txid):
    """Where the recorded settlement landed, as observed on Oct 19 (not proven by the bundle)."""
    if not EVIDENCE.is_file():
        return None
    record = json.loads(EVIDENCE.read_text())
    for run in (record, record.get('browser_video_run', {})):
        block = run.get('settlement_block')
        if run.get('settlement_txid') == txid and block:
            return {'height': block['height'], 'confirmations': run.get('confirmations'),
                    'observed_through': 'one lightwalletd provider (testnet.zec.rocks), Oct 19'}
    return None


def trail(bundle, authority):
    bundle, authority = pathlib.Path(bundle), pathlib.Path(authority)
    manifest = json.loads(read_public(bundle, 'bundle.json', 16384))
    files, data = {}, {}
    for name, limit in LIMITS.items():
        raw = read_public(bundle, name, limit)
        digest = hashlib.sha256(raw).hexdigest()
        if digest != manifest['artifacts'][name]:
            raise RuntimeError('Public bundle file differs from bundle.json: ' + name)
        files[name] = {'bytes': len(raw), 'sha256': digest}
        data[name] = raw
    trusted = json.loads(read_public(authority.parent, authority.name, 16384))
    load = lambda name: json.loads(data[name])
    book, certificate = load('receipt-book.json'), load('final-book-certificate.json')
    native, identity = load('native-public.json'), load('nu7-native-identity.json')
    parsed = parsed_transaction(data['funded-nu7-transaction.bin'])
    actions = parsed['native_actions']
    statement = fields(data['mpc-public/public-input0'], 232)
    receipt = fields(data['receipt-proof-public/public-input0'], 187)
    root = int(book['root'])
    commitments = [int(value) for value in book['commitments']]
    operators = []
    for endorsement in certificate['endorsements']:
        signed = endorsement['book']
        operators.append({
            'operator': 'ABC'[endorsement['operator']], 'file': 'final-book-certificate.json',
            'trusted_key': bytes(trusted['operator_keys'][endorsement['operator']]).hex(),
            'signature': endorsement['signature'],
            'signed_root_matches': int.from_bytes(bytes(signed['accepted_root']), 'big') == root,
            'signed_commitments_match': [int.from_bytes(bytes(entry['bid_commitment']), 'big')
                                         for entry in signed['entries']] == commitments,
        })
    rows = []
    for i in range(6):
        action = native['action_indices'][i]
        cmx = bytes(native['cmxs'][i]).hex()
        rows.append({
            'row': i, 'payee': f'Refund · ticket {i + 1}' if i < 4 else ROLES[i - 4], 'file': 'native-public.json',
            'row_hash': row_hex(native['row_hashes'][i]),
            'row_hash_in_auction_proof': statement[184 + 8 * i:192 + 8 * i] == native['row_hashes'][i],
            'action_index': action, 'action_index_in_auction_proof': statement[175 + i] == action,
            'cmx': cmx, 'cmx_in_transaction_bytes_at_that_action': cmx == actions[action],
            'halo2_proof': {'file': f'native-binding-proof-{i}', **files[f'native-binding-proof-{i}']},
        })
    return {
        'scope': 'Read from the committed public bundle and trusted authority only. The checks are consistency '
                 'checks between these files and the transaction bytes, not proofs; the verifier checks the proofs '
                 'and signatures.',
        'bundle': {'path': bundle.relative_to(ROOT).as_posix() if bundle.is_relative_to(ROOT) else bundle.name,
                   'authority': authority.relative_to(ROOT).as_posix() if authority.is_relative_to(ROOT) else authority.name,
                   'files': len(files), 'all_digests_match_bundle_json': True, 'sdk_pin': manifest['sdk']},
        'book': {
            'file': 'receipt-book.json',
            'commitments': [{'ticket': i + 1, 'commitment': field_hex(value), 'in_auction_proof': statement[164 + i] == value}
                            for i, value in enumerate(commitments)],
            'accepted_root': field_hex(root), 'root_in_auction_proof': statement[168] == root,
            'root_in_receipt_proof': receipt[168] == root, 'operators': operators,
        },
        'price': {
            'file': 'mpc-public/public-input0',
            'price': statement[181], 'sold': statement[182], 'units': statement[160], 'reserve': statement[161],
            'budget': statement[162], 'processing_fee': statement[163],
            'terms_match_authority': statement[160:164] == [trusted['context'][k] for k in
                                                            ('units', 'reserve', 'budget', 'processing_fee')],
            'receipt_proof_same_price_and_sold': receipt[177:179] == statement[181:183],
            'plan_commitment': field_hex(statement[183]), 'public_inputs': len(statement),
            'proof': {'file': 'mpc-public/proof.0', **files['mpc-public/proof.0']},
            'verifying_key': {'file': 'mpc-public/vk', **files['mpc-public/vk']},
            'proof_system': 'UltraHonk (Barretenberg), proved jointly by three parties with coNoir REP3',
        },
        'payouts': rows,
        'transaction': {
            'file': 'funded-nu7-transaction.bin', 'txid': parsed['txid'], 'branch': parsed['branch'],
            'bytes': parsed['bytes'], 'actions': len(actions), 'sha256': files['funded-nu7-transaction.bin']['sha256'],
            'txid_from_bytes_matches_bundle': parsed['txid'] == identity['txid'],
            'actions_from_bytes_match_bundle': actions == [bytes(a).hex() for a in identity['native_actions']],
            'recorded_block': recorded_block(parsed['txid']),
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('bundle', type=pathlib.Path)
    parser.add_argument('authority', type=pathlib.Path)
    args = parser.parse_args()
    print(json.dumps(trail(args.bundle.resolve(), args.authority.resolve()), indent=2))


if __name__ == '__main__':
    main()
