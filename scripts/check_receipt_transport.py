#!/usr/bin/env python3
"""Proof-backed private ticket receipts through native encrypted Zcash memos.

Fabricated funding; a separate one-input/four-output transport transaction, not
the auction's six-output settlement. Native authorization and per-ticket proof
openings are checked separately. No chain, RPC or network service is used.
"""
import argparse
import json
import os
import pathlib
import subprocess
import time
from check_ticket_receipts import public_book
from regtest_deposits import ROOT, ENV, private_read, private_write
from regtest_prepared_gate import write_bytes
from regtest_resources import guarded_json_call

INPUT = ROOT / 'target/release/darkcall-input-tool'
SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'


def call(command):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, env=ENV,
                            capture_output=True, timeout=120)
    if result.returncode:
        diagnostic = ROOT / '.local' / f'receipt-component-failure-{time.time_ns()}.log'
        write_bytes(diagnostic, result.stderr[:65536])
        raise RuntimeError(f'Receipt component rejected; private diagnostic: {diagnostic.name}')
    return json.loads(result.stdout)


def reject(command, diagnostic):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, env=ENV,
                            capture_output=True, timeout=120)
    if result.returncode == 0 or diagnostic not in result.stderr.decode('utf-8', errors='replace'):
        log = ROOT / '.local' / f'receipt-rejection-failure-{time.time_ns()}.log'
        write_bytes(log, result.stderr[:65536])
        raise RuntimeError('Expected specific receipt substitution rejection')


def fixed_verify(public, log):
    with log.open('xb') as output:
        result = subprocess.run(['python3', str(ROOT / 'scripts/verify_external.py'), str(public),
            '--circuit', 'delivery_flags4', '--output-label', log.parent.name + '-' + log.stem], cwd=ROOT, env=ENV,
            stdout=output, stderr=subprocess.STDOUT, timeout=60)
    if result.returncode:
        raise RuntimeError('Fixed C++ flags proof rejected before consuming receipt')


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('book', type=pathlib.Path)
    parser.add_argument('delivery', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected fresh simple ignored transport name')
    local = (ROOT / '.local').resolve()
    book, delivery = args.book.resolve(strict=True), args.delivery.resolve(strict=True)
    if any(not path.is_relative_to(local) for path in (book, delivery)):
        raise RuntimeError('Expected ignored synthetic fixture sources')
    target = local / args.name
    target.mkdir(mode=0o700)
    public = target / 'proof-public'
    public.mkdir(mode=0o700)
    book_bytes = public_book(book / 'book.json')
    write_bytes(public / 'book.json', book_bytes)
    for filename in ('vk', 'proof.0', 'public-input0', 'tampered-price', 'tampered-root', 'tampered-cipher', 'tampered-winner'):
        write_bytes(public / filename, private_read(delivery / filename, 1048576))
    started = time.monotonic()
    fixed_verify(public, target / 'producer-proof.log')
    expected = json.loads(private_read(delivery / 'delivered-amounts.json', 4096))
    outcomes = []
    for index in range(4):
        memo = target / f'ticket-{index}.memo'
        subprocess.run([str(INPUT), 'flags-ticket-memo-export', str(public / 'book.json'),
            str(delivery / 'plan/masks.json'), str(delivery / 'plan/winner-masks.json'),
            str(public / 'public-input0'), str(index), str(memo)], cwd=ROOT, env=ENV,
            check=True, stdout=subprocess.DEVNULL, timeout=30)
        # Independent decode of the exact canonical memo, not a detached status.
        opening = call([INPUT, 'flags-ticket-memo-open', public / 'book.json', index, memo, public / 'public-input0'])
        if opening['refund'] != expected['amounts'][index] or opening['won'] != expected['winners'][index]:
            raise RuntimeError('Producer memo differs from proved private output')
        outcomes.append(opening)
    private_write(target / 'receipt-transport-request.json', {'format': 1, 'amounts': [v['refund'] for v in outcomes]})
    native = guarded_json_call([str(SDK), 'receipt-transport', str(target)], cwd=ROOT, env=ENV,
        timeout=600, observations=target / 'native-resources.jsonl')
    private_write(target / 'native-summary.json', native)
    tx = private_read(target / 'synthetic-receipt-transport.bin', 100000)
    negatives = []
    for index in range(4):
        recipient = target / f'recipient-{index}'
        recipient.mkdir(mode=0o700)
        write_bytes(recipient / 'book.json', book_bytes)
        write_bytes(recipient / 'synthetic-receipt-transport.bin', tx)
        own_name = f'private-recipient-{index}.json'
        own = private_read(target / own_name, 4096)
        write_bytes(recipient / own_name, own)
        fixed_verify(public, recipient / 'fixed-proof.log')
        memo = recipient / 'private-opened.memo'
        receipt = call([SDK, 'receipt-transport-open', recipient, index, memo])
        actual = call([INPUT, 'flags-ticket-memo-open', recipient / 'book.json', index, memo, public / 'public-input0'])
        if (private_read(memo,512) != private_read(target / f'ticket-{index}.memo',512)
                or actual != outcomes[index] or receipt['amount'] != actual['refund']):
            raise RuntimeError('Recipient native/memo/proof opening mismatch')
        if {p.name for p in recipient.glob('private-recipient-*.json')} != {own_name}:
            raise RuntimeError('Recipient received another recipient key')
        base = private_read(memo,512)
        # Reject wrong marker/version, ticket/root/pads, noncanonical scalars,
        # hidden extra disclosures and truncated/trailing serialization.
        cases = []
        for label, position in [('marker',0),('version',8),('ticket',9),('root',72),
                ('refund-mask',104),('refund-blind',136),('winner-mask',168),('winner-blind',200),('padding',201)]:
            changed = bytearray(base)
            changed[position] ^= 1
            cases.append((label, bytes(changed)))
        changed = bytearray(base)
        changed[41:73] = bytes([255])*32
        cases.extend([('noncanonical-field', bytes(changed)), ('truncated', base[:-1]), ('trailing', base+b'\x00')])
        for label, changed in cases:
            path = recipient / (label + '.memo')
            write_bytes(path, changed)
            diagnostic = {'ticket':'expected ticket','root':'root mismatch',
                'refund-mask':'mask/blinder','refund-blind':'mask/blinder',
                'winner-mask':'mask/blinder','winner-blind':'mask/blinder',
                'noncanonical-field':'noncanonical', 'trailing':'bounded regular'}.get(label, 'length/version/padding')
            reject([INPUT,'flags-ticket-memo-open',recipient/'book.json',index,path,public/'public-input0'],diagnostic)
            negatives.append(f'{index}:{label}')
        for label in ('wrong-key','wrong-amount','wrong-txid','wrong-index','extra-key-field','trailing-tx','binding-signature'):
            attack = recipient / label
            attack.mkdir(mode=0o700)
            meta = json.loads(own)
            altered_tx = tx
            if label == 'wrong-key':
                other = json.loads(private_read(target / f'private-recipient-{(index+1)%4}.json',4096))
                meta['viewing_key'] = other['viewing_key']
            elif label == 'wrong-amount': meta['amount'] = meta['amount']-1 if meta['amount'] else 1
            elif label == 'wrong-txid': meta['txid'] = '00'*32
            elif label == 'wrong-index': meta['action_index'] = (meta['action_index']+1)%4
            elif label == 'extra-key-field': meta['won'] = True
            elif label == 'trailing-tx': altered_tx += b'\x00'
            elif label == 'binding-signature': altered_tx = tx[:-1] + bytes([tx[-1]^1])
            private_write(attack / own_name, meta)
            write_bytes(attack / 'synthetic-receipt-transport.bin', altered_tx)
            out = attack / 'must-not-exist.memo'
            diagnostic = {'wrong-key':'cannot decrypt','wrong-amount':'note mismatch','wrong-txid':'identity/shape',
                'wrong-index':'cannot decrypt','extra-key-field':'schema','trailing-tx':'trailing data',
                'binding-signature':'signatures invalid'}[label]
            reject([SDK,'receipt-transport-open',attack,index,out],diagnostic)
            if out.exists(): raise RuntimeError('Rejected recipient request emitted a private memo')
            negatives.append(f'{index}:{label}')
    summary = {'scope':'synthetic native memo transport plus actual fixed MPC receipt proof; separate from auction settlement',
        'normal_cpp_proof_verified_by_producer_and_four_recipients':True,'native':native,
        'recipients': [{'index':i,'won':v['won'],'refund':v['refund']} for i,v in enumerate(outcomes)],
        'substitutions_rejected':negatives,'seconds':time.monotonic()-started,
        'other_recipient_keys_or_operator_pads_read_by_recipient':False,
        'auction_native_output_binding_verified':False,'chain_funding':False,'broadcast':False,'network_service_used':False}
    private_write(target / 'summary.json', summary)
    print(json.dumps(summary,indent=2))


if __name__ == '__main__':
    main()
