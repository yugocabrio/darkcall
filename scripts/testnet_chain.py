#!/usr/bin/env python3
"""Public Zcash testnet access for the funded auction drivers.

Every call goes through the pinned NU7 SDK, which re-checks the chain name,
the testnet genesis and the NU7 branch on each connection. The lightwalletd
provider is trusted for chain data, as the local Zebra node is on regtest; the
network itself validates every broadcast. Keys and wallet files stay in .local.
"""
import json
import os
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV, private_read, private_write

SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'
ENDPOINT = 'https://testnet.zec.rocks:443'
BANK = ROOT / '.local/testnet-bank-20261019'
# Blocks arrive about every 25 s under NU7; this bounds every confirmation wait.
CONFIRMATION_TIMEOUT = 1800
# Public testnet captures run near 10 KB a block and the scanner reads at most
# 100 MB, so the bank rescans only from a birthday this recent (about 35 hours).
BANK_SCAN_BLOCKS = 5000


def sdk(*args, timeout=900):
    return json.loads(subprocess.check_output([str(SDK), *map(str, args)], cwd=ROOT, env=ENV,
                                              text=True, timeout=timeout))


def status():
    return sdk('testnet-status', ENDPOINT, timeout=120)


def tree_at_tip():
    """Frontier snapshot at the current tip, the birthday of a fresh wallet."""
    return sdk('testnet-tree', ENDPOINT, 'tip', timeout=120)


def capture(wallet, end=None):
    """Scan WALLET from its birthday to the current tip, or to block END.
    Returns (scan, notes path, tip)."""
    result = sdk('testnet-capture', wallet, ENDPOINT, *([end] if end else []))
    return result['scan'], pathlib.Path(result['notes']), {'blocks': result['tip_height'], 'tip': result['tip_hash']}


def bank_notes(wallet):
    """The bank's spendable notes as (scan, notes path). A recent birthday is
    rescanned to the tip. An older one is not: the bank's latest scan is used,
    because an anchor from any past block stays valid. source_check confirms that
    scan is still on the best chain before the deposit is built, and a note spent
    since would only make the network refuse the deposit. advance_birthday keeps
    the birthday recent after each funding."""
    birthday = json.loads(private_read(wallet / 'nu7-birthday.json'))['height']
    if status()['blocks'] - birthday <= BANK_SCAN_BLOCKS:
        scan, notes, _ = capture(wallet)
        return scan, notes
    scans = sorted(wallet.glob('private-nu7-notes-*.json'), key=lambda p: int(p.stem.rsplit('-', 1)[1]))
    if not scans:
        raise RuntimeError('The bank has no scan to spend from; rescan it')
    held = json.loads(private_read(scans[-1]))['notes']
    return {'balance_zatoshi': sum(n['amount'] for n in held), 'unspent_notes': len(held),
            'from_scan_at': int(scans[-1].stem.rsplit('-', 1)[1])}, scans[-1]


def rebase(wallet, txid, end=None):
    """Start WALLET's scans at the block before TXID, which spent all it owned
    before, and scan to END or the tip. The old birthday is kept beside the new
    one. Returns the new birthday and balance."""
    record = sdk('testnet-tx', ENDPOINT, txid, timeout=120)
    if not record.get('mined'):
        raise RuntimeError('Transaction is not mined; the wallet keeps its birthday')
    path = wallet / 'nu7-birthday.json'
    old = json.loads(private_read(path))
    height = record['height'] - 1
    if height <= old['height']:
        raise RuntimeError('The wallet already starts at or after that block')
    backup = wallet / f"nu7-birthday-{old['height']}.json"
    if not backup.exists():
        private_write(backup, old)
    staged = wallet / f'nu7-birthday-{height}.json'
    if not staged.exists():
        private_write(staged, sdk('testnet-tree', ENDPOINT, height, timeout=120))
    os.replace(staged, path)
    scan, notes, _ = capture(wallet, end)
    return {'birthday': height, 'balance_zatoshi': scan['balance_zatoshi'], 'notes': notes.name}


def advance_birthday(wallet, notes, txid):
    """After a deposit TXID that spent the bank's only note, rebase the bank on
    it: its change is then all the bank owns. Returns None when the bank held
    other notes and keeps its birthday."""
    if len(json.loads(private_read(notes))['notes']) != 1:
        return None
    return rebase(wallet, txid)


def source_check(notes):
    """The scanned notes' snapshot is canonical, the anchor matches, and every
    note is in its mined block with at least two confirmations."""
    return sdk('testnet-source-check', ENDPOINT, notes, timeout=300)


def send(transaction):
    return sdk('testnet-send', ENDPOINT, transaction, timeout=180)


def wait_confirmations(txid, need=2, timeout=CONFIRMATION_TIMEOUT):
    """Poll until TXID is mined in the best chain with NEED confirmations."""
    deadline = time.monotonic() + timeout
    while True:
        record = sdk('testnet-tx', ENDPOINT, txid, timeout=120)
        if record.get('mined') and record['confirmations'] >= need:
            return record
        if time.monotonic() > deadline:
            raise RuntimeError(f'Transaction {txid[:12]} lacks {need} testnet confirmations; preserve state and recheck')
        time.sleep(15)


if __name__ == '__main__':
    # Recovery: the bank's latest scan predates a deposit that spent its note.
    #   python3 -B scripts/testnet_chain.py rebase-bank TXID
    # starts the bank at the block before TXID and scans three blocks past it, so
    # the change note gets a witness at that anchor without a long capture.
    import sys
    if len(sys.argv) != 3 or sys.argv[1] != 'rebase-bank':
        raise SystemExit('usage: testnet_chain.py rebase-bank DEPOSIT_TXID')
    height = sdk('testnet-tx', ENDPOINT, sys.argv[2], timeout=120)['height']
    print(json.dumps(rebase(BANK, sys.argv[2], height + 2)))
