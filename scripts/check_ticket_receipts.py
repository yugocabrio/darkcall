#!/usr/bin/env python3
"""Per-ticket v2 winner/refund openings authenticated by the fixed public proof.

The operator exports one receipt from private pads; each recipient process reads
only its own receipt and public book/proof. No network delivery, admission or
identity authentication is claimed by this local synthetic integration test.
"""
import argparse
import json
import os
import pathlib
import subprocess
import stat
import time
from regtest_deposits import ROOT, ENV, NATIVE, private_read, private_write
from regtest_prepared_gate import write_bytes
from replay_nu7_auction import write_existing
from regtest_resources import guarded_json_call

INPUT = ROOT / 'target/release/darkcall-input-tool'


def public_book(path):
    """Books contain public commitments; historical ones need not be mode0600."""
    fd = os.open(path,os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd,'rb') as file:
        meta = os.fstat(file.fileno())
        if not stat.S_ISREG(meta.st_mode) or meta.st_nlink != 1 or meta.st_size > 32768:
            raise RuntimeError('Public book type/links/size differ')
        data = file.read(32769)
        if len(data) > 32768:
            raise RuntimeError('Public book exceeds bound')
        return data


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('book', type=pathlib.Path)
    parser.add_argument('delivery', type=pathlib.Path)
    parser.add_argument('--native-bundle', type=pathlib.Path,
                        help='Optional completed public NU6.3 native settlement bundle for this exact auction')
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or len(args.name) > 50 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        raise RuntimeError('Expected fresh simple ignored receipt check name')
    local = (ROOT / '.local').resolve()
    book, delivery = args.book.resolve(strict=True), args.delivery.resolve(strict=True)
    if any(not path.is_relative_to(local) for path in (book, delivery)):
        raise RuntimeError('Expected ignored synthetic fixture sources')
    target = local / args.name
    target.mkdir(mode=0o700)
    public = target / 'proof-public'
    public.mkdir(mode=0o700)
    for filename in ('vk', 'proof.0', 'public-input0', 'tampered-price', 'tampered-root', 'tampered-cipher', 'tampered-winner'):
        write_bytes(public / filename, private_read(delivery / filename, 1048576))
    book_bytes = public_book(book / 'book.json')
    statement = private_read(public / 'public-input0', 187*32)
    if len(statement) != 187*32:
        raise RuntimeError('Ticket status needs exact blinded v2 flags transcript')
    native = None
    started = time.monotonic()
    if args.native_bundle:
        bundle = args.native_bundle.resolve(strict=True)
        if not bundle.is_relative_to(local):
            raise RuntimeError('Native public bundle outside ignored fixtures')
        native = guarded_json_call([str(NATIVE), '--verify-integrated', str(bundle)],
            cwd=ROOT, env=ENV, timeout=900,
            observations=target / 'native-replay-resources.jsonl')
        mpc_run = pathlib.Path(json.loads(private_read(bundle / 'mpc-result.json',4096))['run']).resolve(strict=True)
        if not mpc_run.is_relative_to(local):
            raise RuntimeError('Native MPC public path outside ignored fixtures')
        encoded = private_read(mpc_run / 'public-input0',232*32)
        if (len(encoded) != 232*32 or encoded[:169*32] != statement[:169*32]
                or encoded[181*32:183*32] != statement[177*32:179*32]):
            raise RuntimeError('Recipient transcript differs from verified native auction')
    expected = json.loads(private_read(delivery / 'delivered-amounts.json',4096))
    outcomes = []
    rejections = {}
    for index in range(4):
        recipient = target / ('recipient-' + str(index))
        recipient.mkdir(mode=0o700)
        write_bytes(recipient / 'public-book.json',book_bytes)
        with (recipient / 'fixed-verifier.log').open('xb') as log:
            verified = subprocess.run(['python3', str(ROOT / 'scripts/verify_external.py'),str(public),
                '--circuit','delivery_flags4','--output-label','receipt-' + str(index)],
                cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT, timeout=60)
        if verified.returncode:
            raise RuntimeError('Recipient fixed C++ proof verification failed')
        receipt = recipient / 'private-ticket-receipt.json'
        subprocess.run([str(INPUT),'flags-ticket-export',str(recipient / 'public-book.json'),
            str(delivery / 'plan/masks.json'),str(delivery / 'plan/winner-masks.json'),
            str(public / 'public-input0'),str(index),str(receipt)],cwd=ROOT,env=ENV,check=True,
            stdout=subprocess.DEVNULL,timeout=30)
        command = [str(INPUT),'flags-ticket-open',str(recipient / 'public-book.json'),str(index),str(receipt),str(public / 'public-input0')]
        result = subprocess.run(command,cwd=ROOT,env=ENV,capture_output=True,timeout=30)
        if result.returncode:
            raise RuntimeError('Recipient private opening rejected')
        opened = json.loads(result.stdout)
        if (opened['won'] != expected['winners'][index] or opened['refund'] != expected['amounts'][index]
                or opened['price'] != expected['price'] or opened['sold'] != expected['sold']):
            raise RuntimeError('Recipient opening differs from independently decoded operator output')
        outcomes.append({'ticket_index':index,'won':opened['won'],'refund':opened['refund'],
                         'fixed_cpp_verified':True,'recipient_reads_other_pads':False})
        private_write(recipient / 'private-status.json',opened)
        original = private_read(receipt,4096)
        for label in ('winner-blinder','refund-mask','other-ticket','old-domain','wrong-root','claimed-status'):
            changed = json.loads(original)
            if label == 'winner-blinder':
                changed['winner_pad'][1] = str(int(changed['winner_pad'][1]) ^ 1)
            elif label == 'refund-mask':
                changed['refund_pad'][0] = str(int(changed['refund_pad'][0]) ^ 1)
            elif label == 'other-ticket':
                changed['ticket'] = json.loads(book_bytes)['ids'][(index+1)%4]
            elif label == 'old-domain':
                changed['rule'] = 'private-ticket-opening-delivery-flags-v1'
            elif label == 'wrong-root':
                changed['accepted_root'] = '0'
            else:
                changed['won'] = not opened['won']
            try:
                write_existing(receipt,json.dumps(changed).encode())
                refused = subprocess.run(command,cwd=ROOT,env=ENV,capture_output=True,timeout=30)
                write_bytes(recipient / (label + '-rejected.log'),refused.stdout+refused.stderr)
                messages = {'winner-blinder':b'mask/blinder/context/kind mismatch',
                            'refund-mask':b'mask/blinder/context/kind mismatch',
                            'other-ticket':b"recipient's expected ticket",
                            'old-domain':b'version/domain/root mismatch',
                            'wrong-root':b'version/domain/root mismatch',
                            'claimed-status':b'unknown field'}
                if refused.returncode == 0 or messages[label] not in refused.stderr:
                    raise RuntimeError('Expected receipt substitution refusal absent')
            finally:
                write_existing(receipt,original)
            rejections[str(index) + '-' + label] = True
    summary = {'scope':'local synthetic per-ticket proof-authenticated winner/refund opening',
               'passed':True,'outcomes':outcomes,'receipt_substitutions_rejected':len(rejections),
               'normal_cpp_price_root_cipher_winner_tampering_rejected':True,
               'native_public_settlement_linked':native is not None,'native_replay':native,
               'operator_exports_from_all_pads':True,'recipient_reads_only_own_pads':True,
               'receipt_is_operator_signature':False,'admission_or_funding_authenticated':False,
               'bidder_identity_or_network_delivery_implemented':False,
               'maximum_bids_read_in_verifier':False,'seconds':time.monotonic()-started}
    private_write(local / (args.name + '-summary.json'),summary)
    print(json.dumps(summary,indent=2))


if __name__ == '__main__':
    main()
