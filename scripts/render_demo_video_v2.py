#!/usr/bin/env python3
"""Render the edited demo video from ONE captured real auction.

Every frame is a still from tools/demo-capture/capture.mjs, checked against the
committed capture manifest by SHA-256. Time cuts state the real elapsed time
between the two captured states, computed from the manifest, never typed by
hand. Captions and cards are drawn by headless Brave with the page's own fonts
(tools/demo-capture/overlays.mjs). No proving, keys, uploads or voice synthesis.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
MEDIA = ROOT / 'demo/media/v2'
STORY = ROOT / 'demo/video-storyboard-v2.json'
OVERLAYS = ROOT / 'tools/demo-capture/overlays.mjs'


def duration_text(seconds):
    minutes, rest = divmod(round(seconds), 60)
    return f'{minutes} min {rest:02d} s' if minutes else f'{rest} s'


def render(output):
    ffmpeg, ffprobe, node = shutil.which('ffmpeg'), shutil.which('ffprobe'), shutil.which('node')
    if not ffmpeg or not ffprobe or not node or not OVERLAYS.is_file():
        raise RuntimeError('FFmpeg, Node or the overlay renderer is missing')
    story = json.loads(STORY.read_text())
    manifest = json.loads((MEDIA / 'manifest.json').read_text())
    if story['format'] != 2 or not manifest.get('completed'):
        raise RuntimeError('Storyboard or capture manifest is not a completed v2 run')
    shots = {shot['name']: shot for shot in manifest['shots']}
    facts = manifest['facts']
    local = ROOT / '.local'
    if output.parent.resolve() != local.resolve() or output.exists():
        raise RuntimeError('Use a fresh output directly under ignored .local')
    work = local / ('demo-video-v2-' + secrets.token_hex(5)); work.mkdir(mode=0o700)

    chain = 'public Zcash testnet' if manifest.get('chain') == 'testnet' else 'isolated Zcash regtest'
    badge = 'DarkCall · edited from one real auction · ' + chain
    # Pass 1: validate every scene and describe its text. Captions and cards are
    # drawn by the browser with the page's own fonts (IBM Plex), then composited.
    scenes, used, subtitles, elapsed = [], {}, [], 0
    for index, scene in enumerate(story['scenes']):
        seconds = scene['seconds']
        if type(seconds) is not int or not 2 <= seconds <= 15:
            raise RuntimeError('Unbounded scene duration')
        identity = ('Auction ' + facts['auction_id'][:16] + '…'
                    + ('   ·   Settlement tx ' + facts['txid'][:16] + '…' if scene.get('after_settlement') else ''))
        overlay = work / f'overlay-{index:02}.png'
        if 'shot' in scene:
            shot = shots[scene['shot']]
            image = MEDIA / shot['file']
            if hashlib.sha256(image.read_bytes()).hexdigest() != shot['sha256']:
                raise RuntimeError('Frame differs from the captured run: ' + shot['file'])
            used[shot['file']] = shot['sha256']
            spec = {'kind': 'band', 'title': scene['title'], 'text': scene['caption']}
            subtitle = scene['caption']
        else:
            image = None
            if 'cut' in scene:
                start, end = (shots[name] for name in scene['cut'])
                real = end['elapsed_seconds'] - start['elapsed_seconds']
                title_value = 'Time cut · ' + duration_text(real) + ' of real time'
                body_value = scene['caption']
            else:
                title_value, body_value = scene['title'], scene['body']
            spec = {'kind': 'card', 'title': title_value, 'text': body_value}
            subtitle = title_value + ' — ' + body_value.replace('\n', ' ')
        scenes.append({**spec, 'identity': identity, 'badge': badge, 'file': str(overlay),
                       'image': image, 'seconds': seconds})
        stamp = lambda v: f'{v // 3600:02}:{v % 3600 // 60:02}:{v % 60:02},000'
        subtitles.append(f'{index + 1}\n{stamp(elapsed)} --> {stamp(elapsed + seconds)}\n{subtitle}\n')
        elapsed += seconds
    plan = work / 'overlays.json'
    plan.write_text(json.dumps([{k: v for k, v in s.items() if k not in ('image', 'seconds')} for s in scenes]))
    subprocess.run([node, str(OVERLAYS), str(plan)], cwd=ROOT, check=True, timeout=600, capture_output=True)
    # Pass 2: one clip per scene.
    clips = []
    for index, scene in enumerate(scenes):
        clip = work / f'scene-{index:02}.mp4'
        if scene['image'] is not None:
            inputs = ['-loop', '1', '-framerate', '24', '-i', str(scene['image']),
                      '-loop', '1', '-framerate', '24', '-i', scene['file']]
            graph = ['-filter_complex', '[0:v]scale=1920:1080[b];[b][1:v]overlay=0:0,format=yuv420p']
        else:
            inputs = ['-loop', '1', '-framerate', '24', '-i', scene['file']]
            graph = ['-vf', 'scale=1920:1080,format=yuv420p']
        subprocess.run([ffmpeg, '-nostdin', '-hide_banner', '-loglevel', 'error', '-n', *inputs, *graph,
                        '-t', str(scene['seconds']), '-an', '-c:v', 'libx264', '-preset', 'fast',
                        '-crf', '20', '-threads', '2', '-pix_fmt', 'yuv420p', '-r', '24', str(clip)],
                       check=True, timeout=180)
        clips.append(clip)
    listing = work / 'concat.txt'
    listing.write_text(''.join(f"file '{p}'\n" for p in clips))
    subprocess.run([ffmpeg, '-nostdin', '-hide_banner', '-loglevel', 'error', '-n', '-f', 'concat', '-safe', '0',
                    '-i', str(listing), '-c', 'copy', '-movflags', '+faststart', str(output)], check=True, timeout=120)
    info = json.loads(subprocess.check_output([ffprobe, '-v', 'error', '-show_streams', '-show_format', '-of', 'json', str(output)]))
    if abs(float(info['format']['duration']) - elapsed) > .2 or info['streams'][0]['width'] != 1920:
        raise RuntimeError('Rendered duration/shape differs')
    captions = output.with_suffix('.srt')
    captions.write_text('\n'.join(subtitles))
    return {'passed': True, 'seconds': elapsed, 'scenes': len(clips), 'output': str(output), 'captions': str(captions),
            'auction_id': facts['auction_id'], 'txid': facts['txid'], 'video_sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
            'frames_verified_against_manifest': len(used), 'typeface': 'IBM Plex (SIL OFL 1.1) drawn by the browser',
            'audio': False, 'published': False}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    print(json.dumps(render(args.output.absolute()), indent=2))


if __name__ == '__main__':
    main()
