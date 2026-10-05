#!/usr/bin/env python3
"""Copy only public verification artifacts, replay, and reject tx tampering.

Offline fabricated transactions only. No bid shares, masks, refunds, PCZTs,
keys or witness files are copied. This does not establish chain finality.
"""
import argparse
import json
import pathlib
import shutil
import subprocess
import time
from native_layout import read_layout, WORKING, SHARED

ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/release/darkcall-native-gate'


def run(directory, expected_error=None, authority=None):
    start = time.monotonic()
    command = ([str(BIN), '--verify-certified-integrated', str(directory), str(authority)]
               if authority is not None else [str(BIN), '--verify-integrated', str(directory)])
    result = subprocess.run(command,
                            cwd=ROOT, capture_output=True, text=True, timeout=600,
                            env={**__import__('os').environ, 'RAYON_NUM_THREADS': '2'})
    if expected_error is not None:
        if result.returncode == 0 or expected_error not in result.stderr:
            raise RuntimeError('Expected transaction-level rejection: ' + expected_error)
        return {'rejected': True, 'seconds': round(time.monotonic() - start, 3),
                'reason': expected_error}
    if result.returncode != 0:
        raise RuntimeError('Public replay failed; inspect source artifacts: ' + result.stderr)
    return json.loads(result.stdout)


def copy_bundle(source_path, name):
    if not name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in name):
        raise RuntimeError('Expected a simple unused local name')
    source = source_path.resolve()
    local = ROOT / '.local'
    if not source.is_relative_to(local.resolve()):
        raise RuntimeError('Only ignored offline synthetic artifacts are supported')
    read_layout(source)  # Reject invalid/broken symlink metadata before copying.
    dest = local / name
    dest.mkdir(mode=0o700)
    mpc = dest / 'mpc-public'
    mpc.mkdir()
    result = json.loads((source / 'mpc-result.json').read_text())
    original_mpc = pathlib.Path(result['run']).resolve()
    if not original_mpc.is_relative_to(local.resolve()):
        raise RuntimeError('MPC source must be an ignored local run')
    transaction_names = [n for n in ['synthetic-transaction.bin', 'regtest-transaction.bin'] if (source / n).exists()]
    if len(transaction_names) != 1:
        raise RuntimeError('Expected exactly one explicit transaction artifact')
    names = [transaction_names[0], 'native-public.json'] + [f'native-binding-proof-{i}' for i in range(6)]
    if (source / 'native-layout.json').exists():
        names.append('native-layout.json')
    for name in names:
        shutil.copyfile(source / name, dest / name)
    for name in ['vk', 'proof.0', 'public-input0', 'tampered-price', 'tampered-plan', 'tampered-row']:
        shutil.copyfile(original_mpc / name, mpc / name)
    result['run'] = str(mpc)
    (dest / 'mpc-result.json').write_text(json.dumps(result) + '\n')
    return source, dest


def reject_unknown_layouts(directory, authority=None):
    """Public metadata can select only compiled circuits, never supplied keys."""
    marker = directory / 'native-layout.json'
    original = marker.read_bytes() if marker.exists() else None
    rejected = {}
    try:
        for name, value in [
            ('unknown_layout', {'format': 1, 'layout': 'untrusted-circuit'}),
            ('uploaded_vk_metadata', {'format': 1, 'layout': 'working-33-column-v1',
                                      'vk': 'attacker-supplied-key'}),
            ('unknown_layout_version', {'format': 2, 'layout': 'working-33-column-v1'}),
        ]:
            marker.write_text(json.dumps(value) + '\n')
            rejected[name] = run(directory, 'public native layout', authority)
    finally:
        if original is None:
            marker.unlink()
        else:
            marker.write_bytes(original)
    return rejected


def reject_other_compiled_layout(directory, authority=None):
    """A valid alternate layout id still cannot rebind an existing row proof."""
    marker = directory / 'native-layout.json'
    original = marker.read_bytes() if marker.exists() else None
    layout = read_layout(directory)
    try:
        marker.write_text(json.dumps({'format': 1, 'layout': WORKING if layout == SHARED else SHARED}) + '\n')
        return run(directory, 'public native row proofs', authority)
    finally:
        if original is None:
            marker.unlink()
        else:
            marker.write_bytes(original)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    parser.add_argument('--name', required=True)
    parser.add_argument('--wrong-known-layout', action='store_true',
                        help='Also reject a valid alternate compiled layout/VK; adds key preparation time')
    args = parser.parse_args()
    source, dest = copy_bundle(args.source, args.name)
    # A separate directory proves replay does not require accidental reads of
    # masks/decoded fills/native openings in the original integration folder.
    summary = {'public_only_replay': run(dest), 'private_artifacts_copied': False,
               'original_private_directory_required': False,
               'source': source.name, 'directory': dest.name, 'tampering': {}}
    tx_path = next(dest / n for n in ['synthetic-transaction.bin', 'regtest-transaction.bin'] if (dest / n).exists())
    original = tx_path.read_bytes()
    try:
        tx_path.write_bytes(original + b'\x00')
        summary['tampering']['trailing_byte'] = run(dest, 'unexpected transaction format/pools/trailing data')
        changed = bytearray(original)
        changed[-1] ^= 1
        tx_path.write_bytes(changed)
        summary['tampering']['binding_signature'] = run(dest, 'native proof or spend/binding signatures invalid')
    finally:
        tx_path.write_bytes(original)
    statement_path = dest / 'native-public.json'
    original_statement = statement_path.read_bytes()
    try:
        statement = json.loads(original_statement)
        statement['native_actions'][0][0] ^= 1
        statement_path.write_text(json.dumps(statement) + '\n')
        summary['tampering']['transaction_action_metadata'] = run(dest, 'statement actions differ from serialized transaction')
    finally:
        statement_path.write_bytes(original_statement)
    summary['tampering'].update(reject_unknown_layouts(dest))
    if args.wrong_known_layout:
        summary['tampering']['other_compiled_layout'] = reject_other_compiled_layout(dest)
    summary['passed'] = True
    (dest / 'replay-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
