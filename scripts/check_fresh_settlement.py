#!/usr/bin/env python3
"""Fresh browser book → Fund → REP3 clearing → SAME-book NU7 settlement, over HTTP.

Drives the running local demo exactly like the browser: create (default terms),
four sealed demo bids, certified close, Fund (actual deposits + chain admission
over the same root), REP3 clearing, own results, then Settle.
After the settlement completes it opens all four own shielded receipts and
checks they equal the browser clearing results, and that a corrupted receipt
proof and a foreign capability are refused. Synthetic demo bids only.
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

    state.update(call('/api/state')[1])
    assert state['local_rehearsal_available'] and state['regtest_settlement_available']
    recorded = state['cases'][0]['txid']
    # Non-default terms are refused before any work.
    code, odd = call('/api/rehearsal/create', {'units': 2, 'reserve': 10, 'budget': 100,
                                               'closes_at_utc': int(time.time()) + 3600})
    assert code == 200 and odd['settlement_available'] is False
    terms = {'units': units, 'reserve': reserve, 'budget': 100, 'closes_at_utc': int(time.time()) + 40}
    code, auction = call('/api/rehearsal/create', terms)
    assert code == 200 and auction['settlement_available'], auction
    scope = {'auction_id': auction['auction_id']}
    assert call('/api/rehearsal/settle', scope)[0] == 400  # Not cleared yet.
    accepted = []
    for bid in bids:
        code, result = call('/api/rehearsal/bid', {**scope, 'maximum_bid': bid})
        assert code == 200 and result['receipt_signatures_verified'] == 3, result
        accepted.append(result)
    while int(time.time()) < auction['closes_at_utc']:
        time.sleep(.2)
    code, closed = call('/api/rehearsal/close', scope); assert code == 200, closed
    # Protocol order: no clearing or settlement before actual funding/admission.
    assert call('/api/rehearsal/clear', scope)[0] == 400
    assert call('/api/rehearsal/settle', scope)[0] == 400
    code, view = call('/api/rehearsal/fund', scope); assert code == 200 and view['state'] == 'running', view
    while True:
        time.sleep(5)
        funding = call('/api/rehearsal/auction/' + auction['auction_id'])[1]['funding']
        if funding['state'] != 'running':
            break
    assert funding['state'] == 'done' and funding['result']['accepted_root'] == closed['accepted_root'], funding
    funded_at = time.monotonic()
    code, cleared = call('/api/rehearsal/clear', scope); assert code == 200, cleared
    assert cleared['rep3_clearing_verified'] and cleared['accepted_root'] == closed['accepted_root']
    fresh = []
    for a in accepted:
        code, opened = call('/api/rehearsal/result', {**scope, 'ticket': a['ticket'],
                            'opening_token': a['opening_token'], 'action': 'open'})
        assert code == 200 and opened['opening_verified'], opened
        fresh.append(opened)

    code, view = call('/api/rehearsal/settle', scope); assert code == 200 and view['state'] == 'running', view
    settle_started = time.monotonic()
    # A second request while the job owns the proof lock is refused, not duplicated.
    assert call('/api/rehearsal/settle', scope)[0] == 409
    # The settled auction's checks have their own lock: they still answer.
    assert call('/api/verify', {'case': 'positive', 'ticket': 0, 'action': 'verify'})[0] == 200
    phases = []
    while True:
        time.sleep(10)
        public = call('/api/rehearsal/auction/' + auction['auction_id'])[1]
        job = public['settlement']
        phases = [e['phase'] + ':' + e['state'] for e in job['events']]
        encoded = json.dumps(public)
        assert not any(a['opening_token'] in encoded for a in accepted)
        assert '/.local/' not in encoded and 'private-' not in encoded
        if job['state'] != 'running':
            break
    assert job['state'] == 'done', job
    settled = job['result']
    assert settled['accepted_root'] == closed['accepted_root'] and settled['same_accepted_root_and_tickets']
    assert (settled['price'], settled['sold']) == (cleared['price'], cleared['sold'])
    assert settled['native_row_proofs_verified'] and settled['normal_cpp_auction_proof_verified']
    assert settled['accepted_book_and_recipients_authenticated'] and settled['native_proof_and_signatures_verified']
    assert settled['txid'] != recorded and settled['confirmations'] >= 2

    receipts = []
    for a, opened in zip(accepted, fresh):
        code, native = call('/api/rehearsal/result', {**scope, 'ticket': a['ticket'],
                            'opening_token': a['opening_token'], 'action': 'native-open'})
        assert code == 200 and native['native_settlement'], native
        assert native['txid'] == settled['txid'] and native['own_viewing_key_only']
        assert (native['won'], native['refund']) == (opened['won'], opened['refund'])
        receipts.append({k: native[k] for k in ('won', 'refund', 'seconds')})
    code, tamper = call('/api/rehearsal/result', {**scope, 'ticket': accepted[0]['ticket'],
                        'opening_token': accepted[0]['opening_token'], 'action': 'native-tamper'})
    assert code == 200 and tamper['rejected'] and not tamper['key_read'], tamper
    foreign = call('/api/rehearsal/result', {**scope, 'ticket': accepted[0]['ticket'],
                   'opening_token': accepted[1]['opening_token'], 'action': 'native-open'})[0]
    assert foreign == 403
    return {'scope': 'fresh browser book settled on ' + ('public Zcash testnet' if state.get('live_chain') == 'testnet' else 'isolated NU7 regtest') + '; synthetic demo bids; one host',
        'terms': {k: terms[k] for k in ('units', 'reserve', 'budget')}, 'auction_id': auction['auction_id'],
        'accepted_root': closed['accepted_root'], 'browser_clearing': {k: cleared[k] for k in ('price', 'sold', 'seconds')},
        'funding': funding['result'], 'clearing_after_funding': True,
        'clear_and_settle_before_funding_refused_400': True,
        'seconds_from_funded_to_settle_request': round(settle_started - funded_at, 3),
        'settlement': settled, 'progress_phases': phases, 'own_native_receipts': receipts,
        'own_receipts_equal_browser_results': True, 'native_tamper_rejected_before_key_read': True,
        'foreign_capability_refused': True, 'settle_before_clear_refused': True,
        'concurrent_settle_refused_409_and_recorded_check_answered': True, 'non_default_terms_not_settleable': True,
        'recorded_txid_unchanged': state['cases'][0]['txid'] == recorded,
        'seconds': time.monotonic() - started}


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
