#!/usr/bin/env python3
"""Pack the private .local files the hosted demo reads, and nothing else.

The hosted server needs two things that are not in the repository: the
recorded testnet run's receipt material (its receipts open with its own
testnet keys) and the testnet bank that funds new auctions. Only the files
serve_demo.py reads from the recorded run are taken, by name; the bank is
taken whole. Testnet coins and demo keys only. The pack is refused if any text
file in it names this machine's home directory. Logs are never included.
"""
import argparse
import os
import pathlib
import tarfile
from regtest_deposits import ROOT
from testnet_chain import BANK

LOCAL = ROOT / '.local'
# What Demo.state(), Demo.verify() and the funded receipt opener read.
RUN_FILES = ['nu7-native-identity.json', 'funded-nu7-transaction.bin', 'confirmed-summary.json',
             'receipt-book.json', 'receipt-proof-public/vk', 'receipt-proof-public/proof.0',
             'receipt-proof-public/public-input0', *(f'private-funded-recipient-{i}.json' for i in range(4))]


def members(run):
    files = [(LOCAL / run / name, f'{run}/{name}') for name in RUN_FILES]
    files += [(path, str(path.relative_to(LOCAL))) for path in sorted(BANK.rglob('*'))
              if path.is_file() and path.suffix not in ('.log', '.jsonl')]
    home = str(pathlib.Path.home()).encode()
    for path, _ in files:
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f'Missing or linked hosted file: {path.relative_to(LOCAL)}')
        if home in path.read_bytes():
            raise RuntimeError(f'{path.relative_to(LOCAL)} names this machine\'s home directory')
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', default='settle-aa36c45a6a-settlement')
    parser.add_argument('--tar', type=pathlib.Path, help='write the files to this private tar.gz (outside the repository)')
    args = parser.parse_args()
    files = members(args.run)
    print(f'{len(files)} files, {sum(p.stat().st_size for p, _ in files) / 2**20:.0f} MiB '
          f'({len(RUN_FILES)} from {args.run}, the rest from {BANK.name})')
    if args.tar:
        if args.tar.resolve().is_relative_to(ROOT):
            raise RuntimeError('Write the pack outside the repository')
        os.umask(0o077)
        with tarfile.open(args.tar, 'w:gz') as archive:
            for path, name in files:
                archive.add(path, arcname=name, recursive=False)
        print(f'wrote {args.tar}')


if __name__ == '__main__':
    main()
