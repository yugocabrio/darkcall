#!/usr/bin/env python3
"""Check whether this checkout can run the demo and the public verifier.

Read-only. It looks for the pinned tool builds, the committed bundles, the
cached Halo2 parameters, the local regtest node and free disk, and says what
each missing piece is needed for. It exits non-zero only when the public
verifier cannot run. Build instructions: docs/reproduce.md.
"""
import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
# (path, what it is needed for, needed by the public verifier)
TOOLS = [
    ('target/release/darkcall-native-gate', 'verify, settle', True),
    ('.deps/nu7-compat-target/release/darkcall-nu7-compat', 'verify, settle', True),
    ('.deps/barretenberg-src/barretenberg/cpp/build-darkcall/bin/bb', 'verify, bids', True),
    ('.deps/noir-1.0.0-rc.2/nargo', 'verify, test', True),
    ('target/release/darkcall-input-tool', 'bids, settle', False),
    ('target/release/poseidon_batch', 'settle', False),
    ('target/release/examples/regtest_wallet', 'settle', False),
    ('target/release/examples/rehearsal_admission', 'bids', False),
    ('.deps/co-snarks/target/release/plaindriver', 'bids', False),
    ('.deps/co-snarks/target-aes-native/release/co-noir', 'clear, settle', False),
]


def line(ok, text, hint=''):
    print(('  ok   ' if ok else '  --   ') + text + (f'  ({hint})' if hint else ''))


def main():
    blocking = False
    print('Pinned tools')
    for path, use, verifier in TOOLS:
        ok = os.access(ROOT / path, os.X_OK)
        line(ok, path, '' if ok else 'missing; needed for ' + use)
        blocking |= verifier and not ok
    nargo = ROOT / '.deps/noir-1.0.0-rc.2/nargo'
    if os.access(nargo, os.X_OK):
        version = subprocess.run([str(nargo), '--version'], capture_output=True, text=True).stdout
        line('1.0.0-rc.2' in version, 'nargo reports 1.0.0-rc.2')

    print('Committed bundles')
    bundles = sorted(p.name for p in (ROOT / 'demo/artifacts').iterdir() if p.is_dir())
    funded = [b for b in bundles if (ROOT / 'demo/authority' / (b + '.json')).is_file()]
    synthetic = len(bundles) - len(funded)
    line(bool(funded), f'{len(funded)} settlement bundles with their separate authority'
         + (f', {synthetic} synthetic' if synthetic else ''))

    print('Speed and live runs')
    cache = pathlib.Path(os.environ.get('DARKCALL_HALO2_PARAMS', ROOT / '.local/cache/halo2-vesta-k17.params'))
    line(cache.is_file(), 'cached Halo2 parameters',
         '' if cache.is_file() else 'the first check computes them, about 1.5 min, then caches them')
    try:
        from regtest_nu7_node import status
        tip = status()['blocks']
        line(True, f'local NU7 regtest node, tip {tip}')
    except Exception:
        line(False, 'local NU7 regtest node',
             'needed only to run a new auction: python3 scripts/regtest_nu7_node.py start')
    free = shutil.disk_usage(ROOT).free / 1024 ** 3
    line(free >= 10, f'{free:.1f} GiB free disk', '' if free >= 10 else 'funding and settling need 10 GiB')
    line(shutil.which('docker') is not None, 'docker', '' if shutil.which('docker') else 'needed only for make verify-docker')

    print()
    print('The public verifier cannot run: build the tools in docs/reproduce.md.' if blocking
          else 'Ready: make verify checks the recorded testnet settlement with no secrets.')
    return 1 if blocking else 0


if __name__ == '__main__':
    sys.exit(main())
