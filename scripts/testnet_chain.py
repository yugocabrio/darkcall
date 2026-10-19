#!/usr/bin/env python3
"""Public Zcash testnet access for the funded auction drivers.

Every call goes through the pinned NU7 SDK, which re-checks the chain name,
the testnet genesis and the NU7 branch on each connection. The lightwalletd
provider is trusted for chain data, as the local Zebra node is on regtest; the
network itself validates every broadcast. Keys and wallet files stay in .local.
"""
import json
import pathlib
import subprocess
import time
from regtest_deposits import ROOT, ENV

SDK = ROOT / '.deps/nu7-compat-target/release/darkcall-nu7-compat'
ENDPOINT = 'https://testnet.zec.rocks:443'
BANK = ROOT / '.local/testnet-bank-20261019'
# Blocks arrive about every 25 s under NU7; this bounds every confirmation wait.
CONFIRMATION_TIMEOUT = 1800


def sdk(*args, timeout=900):
    return json.loads(subprocess.check_output([str(SDK), *map(str, args)], cwd=ROOT, env=ENV,
                                              text=True, timeout=timeout))


def status():
    return sdk('testnet-status', ENDPOINT, timeout=120)


def tree_at_tip():
    """Frontier snapshot at the current tip, the birthday of a fresh wallet."""
    return sdk('testnet-tree', ENDPOINT, 'tip', timeout=120)


def capture(wallet):
    """Scan WALLET from its birthday to the current tip. Returns (scan, notes path, tip)."""
    result = sdk('testnet-capture', wallet, ENDPOINT)
    return result['scan'], pathlib.Path(result['notes']), {'blocks': result['tip_height'], 'tip': result['tip_hash']}


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
