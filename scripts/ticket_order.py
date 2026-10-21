#!/usr/bin/env python3
"""Record the order in which a recorded auction's bids were sealed.

The settled page lists bidders by this order, so "Bidder 01" there is the
"Ticket 01" the browser showed when the bid was sealed (and the video shows).
The public book itself is sorted by ticket id. The order comes from the
operators' signed acceptance receipts, which the browser room numbers in
acceptance order (accepted-0.json is the first sealed bid). Only public ticket
ids are written; it is a display label, not something the verifier checks.
"""
import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def ticket_order(settled):
    record = json.loads(settled.read_text())
    room = pathlib.Path(record['paths']['book']).parents[1]
    if not room.resolve().is_relative_to((ROOT / '.local').resolve()):
        raise RuntimeError('Expected the recorded browser room under .local')
    order = []
    for index in range(4):
        receipts = json.loads((room / f'accepted-{index}.json').read_text())
        tickets = {bytes(receipt['payload']['ticket']).hex() for receipt in receipts}
        if len(receipts) != 3 or len(tickets) != 1:
            raise RuntimeError('Expected three operator receipts for one ticket')
        order.append(tickets.pop())
    if sorted(order) != sorted(record['public']['tickets']):
        raise RuntimeError('Sealing order does not cover the settled tickets')
    return {'format': 1, 'auction_id': room.name, 'txid': record['public']['txid'],
            'scope': 'Ticket ids in the order the bids were sealed in the browser; display labels only',
            'sealing_order': order}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('settled', type=pathlib.Path, help='.local/settle-<run>-settled.json')
    parser.add_argument('output', type=pathlib.Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(ticket_order(args.settled), indent=2) + '\n')


if __name__ == '__main__':
    main()
