#!/usr/bin/env python3
"""Funded book that cannot settle → refund every deposit, over HTTP.

Drives the running local demo like the browser: create (default terms), four
sealed demo bids, certified close and Fund (actual deposits and chain
admission). Then operator C is taken to have stopped, so the book cannot be
cleared, and Cancel asks operators A and B to refund it: one transaction,
signed 2-of-3 with FROST, returns each deposit to its ticket's certified refund
receiver less an equal share of the network fee. The driver checks every refund
wallet and the empty escrow, and that C's journal and key share were never
opened. Afterwards clearing and settlement of that book are refused, and a
second cancel returns the same job. Synthetic demo bids only.
"""
import argparse
import json
import time
import urllib.error
import urllib.request
from regtest_deposits import ROOT, private_write


def run(port, bids, reserve, units=3):
    origin = 'http://127.0.0.1:' + str(port)
    started = time.monotonic()
    state = {}

    def call(path, value=None, timeout=900):
        headers = {'Origin': origin}
        if value is not None:
            headers.update({'Content-Type': 'application/json', 'X-DarkCall-Token': state['csrf']})
        request = urllib.request.Request(origin + path, headers=headers,
                                         data=None if value is None else json.dumps(value).encode())
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.load(error)

    def wait(auction_id, kind):
        while True:
            time.sleep(5)
            public = call('/api/rehearsal/auction/' + auction_id)[1]
            encoded = json.dumps(public)
            assert '/.local/' not in encoded and 'private-' not in encoded
            if public[kind]['state'] != 'running':
                return public[kind]

    state.update(call('/api/state')[1])
    assert state['local_rehearsal_available'] and state['regtest_settlement_available']
    terms = {'units': units, 'reserve': reserve, 'budget': 100, 'closes_at_utc': int(time.time()) + 40}
    code, auction = call('/api/rehearsal/create', terms)
    assert code == 200 and auction['settlement_available'], auction
    scope = {'auction_id': auction['auction_id']}
    for bid in bids:
        code, result = call('/api/rehearsal/bid', {**scope, 'maximum_bid': bid})
        assert code == 200 and result['receipt_signatures_verified'] == 3, result
    # Nothing to refund before funding.
    assert call('/api/rehearsal/cancel', scope)[0] == 400
    while int(time.time()) < auction['closes_at_utc']:
        time.sleep(.2)
    code, closed = call('/api/rehearsal/close', scope); assert code == 200, closed
    assert call('/api/rehearsal/cancel', scope)[0] == 400
    code, view = call('/api/rehearsal/fund', scope); assert code == 200 and view['state'] == 'running', view
    funding = wait(auction['auction_id'], 'funding')
    assert funding['state'] == 'done' and funding['result']['accepted_root'] == closed['accepted_root'], funding

    cancel_started = time.monotonic()
    code, view = call('/api/rehearsal/cancel', scope); assert code == 200 and view['state'] == 'running', view
    # One heavy job at a time: settling meanwhile is refused.
    assert call('/api/rehearsal/settle', scope)[0] in (400, 409)
    job = wait(auction['auction_id'], 'cancellation')
    assert job['state'] == 'done', job
    refund = job['result']
    assert refund['accepted_root'] == closed['accepted_root'] and refund['refunds'] == 4
    assert refund['refund_each'] == 10100 and refund['deposit_each'] == 15100 and refund['network_fee'] == 20000
    assert refund['signers'] == ['A', 'B'] and refund['stopped_operator'] == 'C'
    assert not refund['stopped_operator_key_or_journal_read'] and refund['threshold_spends'] == 4
    assert refund['escrow_empty'] and refund['confirmations'] >= 2
    assert all(w['balance_zatoshi'] == 10100 for w in refund['refund_wallets_checked'])
    phases = [e['phase'] + ':' + e['state'] for e in job['events']]

    # The refunded book can no longer be cleared or settled; cancel is idempotent.
    assert call('/api/rehearsal/clear', scope)[0] == 400
    assert call('/api/rehearsal/settle', scope)[0] == 400
    code, again = call('/api/rehearsal/cancel', scope)
    assert code == 200 and again['state'] == 'done' and again['result']['txid'] == refund['txid']
    return {'scope': 'funded browser book refunded on ' + ('public Zcash testnet' if state.get('live_chain') == 'testnet'
                     else 'isolated NU7 regtest') + ' after operator C stopped; synthetic demo bids; one host',
        'terms': {k: terms[k] for k in ('units', 'reserve', 'budget')}, 'auction_id': auction['auction_id'],
        'accepted_root': closed['accepted_root'], 'funding': funding['result'], 'refund': refund,
        'progress_phases': phases, 'cancel_before_funding_refused_400': True,
        'clear_and_settle_after_refund_refused_400': True, 'second_cancel_same_job': True,
        'seconds_from_cancel_request': round(time.monotonic() - cancel_started, 3),
        'seconds': round(time.monotonic() - started, 3)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8877)
    parser.add_argument('--bids', default='77,61,43,21')
    parser.add_argument('--reserve', type=int, choices=(0, 20), default=20)
    parser.add_argument('--units', type=int, choices=(1, 2, 3), default=3)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    result = run(args.port, [int(b) for b in args.bids.split(',')], args.reserve, args.units)
    private_write(ROOT / args.output, result)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
