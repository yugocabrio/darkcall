#!/usr/bin/env python3
"""Import one completed browser capture into demo/media/v2.

Input is a directory written by tools/demo-capture/capture.mjs. Every PNG still
is converted to JPEG (quality 88) with sips. The committed manifest keeps the
original PNG SHA-256 next to the JPEG SHA-256, so a frame can be traced back to
the capture. Incomplete runs and unexpected files are refused, and the old
frames are replaced only after every new frame has converted.
"""
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
MEDIA = ROOT / 'demo/media/v2'


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(source):
    manifest = json.loads((source / 'manifest.json').read_text())
    if not manifest.get('completed') or manifest.get('format') != 1:
        raise SystemExit('capture is not a completed format-1 run')
    with tempfile.TemporaryDirectory(dir=ROOT / '.local') as work:
        work = pathlib.Path(work)
        for shot in manifest['shots']:
            png = source / shot['file']
            if png.suffix != '.png' or sha256(png) != shot['sha256']:
                raise SystemExit('frame differs from its capture record: ' + shot['file'])
            jpg = work / (png.stem + '.jpg')
            subprocess.run(['sips', '-s', 'format', 'jpeg', '-s', 'formatOptions', '88', str(png), '--out', str(jpg)],
                           check=True, capture_output=True)
            shot.update(file=jpg.name, png_sha256=shot['sha256'], sha256=sha256(jpg))
        manifest['frames'] = 'JPEG (quality 88) converted from the captured PNGs; png_sha256 is the original capture'
        old = json.loads((MEDIA / 'manifest.json').read_text())
        for shot in old['shots']:
            (MEDIA / shot['file']).unlink(missing_ok=True)
        for jpg in sorted(work.glob('*.jpg')):
            shutil.move(str(jpg), MEDIA / jpg.name)
    (MEDIA / 'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'imported': len(manifest['shots']), 'auction_id': manifest['facts']['auction_id'],
                      'txid': manifest['facts']['txid'], 'total_seconds': manifest['total_seconds']}, indent=2))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: import_demo_capture.py CAPTURE_DIRECTORY')
    main(pathlib.Path(sys.argv[1]).resolve())
