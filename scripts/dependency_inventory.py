#!/usr/bin/env python3
"""Export locked package identities and declared licenses for a selected target.

This records source metadata; it does not certify legal compatibility or audit
upstream software. It never exports local absolute paths or private artifacts.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default='aarch64-apple-darwin')
    parser.add_argument('--manifest', type=pathlib.Path, default=ROOT / 'Cargo.toml',
                        help='Repository Cargo manifest; defaults to the main workspace')
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    manifest = args.manifest.resolve()
    manifest_name = manifest.relative_to(ROOT).as_posix()
    if manifest.name != 'Cargo.toml':
        raise RuntimeError('Expected a repository Cargo.toml')
    lock_bytes = manifest.with_name('Cargo.lock').read_bytes()
    locked = tomllib.loads(lock_bytes.decode())['package']
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline',
        '--manifest-path', str(manifest), '--filter-platform', args.target,
        '--format-version', '1'], cwd=ROOT, text=True))
    packages = []
    for p in metadata['packages']:
        matched = [l for l in locked if (l['name'], l['version'], l.get('source')) ==
                   (p['name'], p['version'], p.get('source'))]
        if len(matched) != 1:
            raise RuntimeError('Metadata package does not uniquely match Cargo.lock')
        source = p.get('source')
        if source is None:
            # Path packages in this workspace and copied vendor source only.
            path = pathlib.Path(p['manifest_path']).resolve().relative_to(ROOT)
            source = 'repository:' + path.as_posix()
        packages.append({'name': p['name'], 'version': p['version'], 'source': source,
            'declared_license': p.get('license'), 'registry_checksum': matched[0].get('checksum')})
    packages.sort(key=lambda p: (p['name'], p['version'], p['source']))
    output = {'scope': 'declared package licenses and locked resolution; no legal compatibility claim',
        'manifest': manifest_name,
        'target': args.target, 'cargo_lock_sha256': hashlib.sha256(lock_bytes).hexdigest(),
        'package_count': len(packages), 'missing_declared_license': [p['name'] for p in packages if not p['declared_license']],
        'tool_pins': ({'nargo': '1.0.0-rc.2',
            'co_snarks': '467a2abbad940d40ed78f38d424cc21920988976',
            'cpp_barretenberg': '87aa5283d5b0cab6632ed49fdd25f6548d2ba91f',
            'zebra': '6.3.0 / f5c5277',
            'runtime_image': 'ubuntu@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55'}
            if manifest == ROOT / 'Cargo.toml' else {}),
        'packages': packages}
    args.output.write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps({k: output[k] for k in ['target', 'package_count', 'missing_declared_license', 'cargo_lock_sha256']}))


if __name__ == '__main__':
    main()
