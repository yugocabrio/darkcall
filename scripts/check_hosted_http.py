#!/usr/bin/env python3
"""Check a demo server started in hosted mode, as the TLS proxy would reach it.

Start the server as the hosted image does, for example
  python3 -B scripts/serve_demo.py --canonical --funded --run settle-aa36c45a6a-settlement \
    --bundle demo/artifacts/testnet-auction-nu7-v2 --authority demo/authority/testnet-auction-nu7-v2.json \
    --ticket-order demo/ticket-order/testnet-auction-nu7-v2.json --chain testnet \
    --public-origin https://darkcall.test --port 8890
then run this with the same origin and port. Requests carry the public Host and an
X-Forwarded-For address, as Caddy sends them. It checks the three pages and the
icons, the hosted flag, a bidder receipt, the proof trail, the fast cheat, the
host / origin / token rejections and the per-address rate limit. It does not
fund or settle a new auction (that spends testnet coins).
"""
import argparse
import http.client
import json
import time
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--origin', default='https://darkcall.test')
    parser.add_argument('--port', type=int, default=8890)
    args = parser.parse_args()
    host = urlsplit(args.origin).netloc
    started = time.monotonic()

    def call(method, path, body=None, headers=None, client='198.51.100.7', timeout=300):
        connection = http.client.HTTPConnection('127.0.0.1', args.port, timeout=timeout)
        try:
            connection.request(method, path, body, {'Host': host, 'X-Forwarded-For': client, **(headers or {})})
            response = connection.getresponse()
            return response.status, response.read(200000)
        finally:
            connection.close()

    checked = []
    for path in ('/', '/demo/settled', '/demo/run', '/app.js', '/style.css', '/favicon.ico', '/icon-gavel.svg'):
        if call('GET', path)[0] != 200:
            raise RuntimeError('Hosted page missing: ' + path)
    checked.append('pages and icons')
    if call('GET', '/', headers={'Host': '127.0.0.1:%d' % args.port})[0] != 403:
        raise RuntimeError('A request for another host was served')
    checked.append('other host refused')
    status, raw = call('GET', '/api/state')
    state = json.loads(raw)
    if status != 200 or state['hosted'] is not True or not state['funded_canonical'] or state['live_chain'] != 'testnet':
        raise RuntimeError('Hosted state differs')
    checked.append('hosted state')
    headers = {'Origin': args.origin, 'Content-Type': 'application/json', 'X-DarkCall-Token': state['csrf']}
    for label, extra in (('no origin', {'Origin': ''}), ('other origin', {'Origin': 'https://attacker.invalid'}),
                         ('no token', {'X-DarkCall-Token': ''})):
        if call('POST', '/api/verify', json.dumps({'case': 'positive', 'ticket': 0, 'action': 'verify'}),
                {**headers, **extra})[0] != 403:
            raise RuntimeError('Write accepted with ' + label)
    checked.append('origin and token required')
    status, raw = call('POST', '/api/verify', json.dumps({'case': 'positive', 'ticket': 1, 'action': 'verify'}), headers)
    receipt = json.loads(raw)
    if status != 200 or not receipt['own_ticket_opening_verified'] or receipt['txid'] != state['cases'][0]['txid']:
        raise RuntimeError('Hosted receipt did not open')
    checked.append('bidder receipt')
    status, raw = call('GET', '/api/trail')
    if status != 200 or json.loads(raw)['transaction']['txid'] != state['cases'][0]['txid']:
        raise RuntimeError('Hosted proof trail differs')
    checked.append('proof trail')
    status, raw = call('POST', '/api/cheat', json.dumps({'kind': 'book'}), headers)
    cheat = json.loads(raw)
    if status != 200 or cheat['outcome'] != 'refused':
        raise RuntimeError('Hosted cheat was not refused')
    checked.append('cheat refused')
    # Rate limit: the receipt bucket allows 12 per address per 10 minutes; malformed
    # requests count too, so these cost nothing. Another address is unaffected.
    codes = [call('POST', '/api/verify', '{}', headers, client='198.51.100.9')[0] for _ in range(13)]
    if codes[:12] != [400] * 12 or codes[12] != 429:
        raise RuntimeError('Per-address limit differs: %s' % codes)
    if call('POST', '/api/verify', '{}', headers, client='198.51.100.10')[0] != 400:
        raise RuntimeError('One address limited another')
    checked.append('per-address rate limit')
    print(json.dumps({'scope': 'hosted mode over loopback, as the TLS proxy forwards it; no new auction funded',
                      'checked': checked, 'seconds': round(time.monotonic() - started, 1)}, indent=2))


if __name__ == '__main__':
    main()
