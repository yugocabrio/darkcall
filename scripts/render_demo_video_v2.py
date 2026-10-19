#!/usr/bin/env python3
"""Render the edited demo video from ONE captured real auction.

Every frame is a still from tools/demo-capture/capture.mjs, checked against the
committed capture manifest by SHA-256. Time cuts state the real elapsed time
between the two captured states, computed from the manifest, never typed by
hand. No proving, keys, uploads, voice synthesis or browser automation here.
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


def escaped(path):
    return str(path).replace('\\', '\\\\').replace(':', '\\:').replace("'", "'\\''")


def duration_text(seconds):
    minutes, rest = divmod(round(seconds), 60)
    return f'{minutes} min {rest:02d} s' if minutes else f'{rest} s'


def render(output, font, bold):
    ffmpeg, ffprobe = shutil.which('ffmpeg'), shutil.which('ffprobe')
    if not ffmpeg or not ffprobe or not font.is_file() or not bold.is_file():
        raise RuntimeError('FFmpeg/font prerequisites missing')
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

    def text(name, value):
        path = work / name; path.write_text(value); return path

    def draw(path, y, size, color='0xffffff', x='80', face=None):
        return (f"drawtext=fontfile='{escaped(face or font)}':textfile='{escaped(path)}':expansion=none:"
                f"fontsize={size}:fontcolor={color}:x={x}:y={y}:line_spacing=10")

    badge = text('badge.txt', 'DarkCall · edited from one real auction · isolated Zcash regtest')
    clips, used, subtitles, elapsed = [], {}, [], 0
    for index, scene in enumerate(story['scenes']):
        seconds = scene['seconds']
        if type(seconds) is not int or not 2 <= seconds <= 15:
            raise RuntimeError('Unbounded scene duration')
        identity = text(f'id-{index}.txt', 'Auction ' + facts['auction_id'][:16] + '…'
                        + ('   ·   Settlement tx ' + facts['txid'][:16] + '…' if scene.get('after_settlement') else ''))
        clip = work / f'scene-{index:02}.mp4'
        if 'shot' in scene:
            shot = shots[scene['shot']]
            image = MEDIA / shot['file']
            if hashlib.sha256(image.read_bytes()).hexdigest() != shot['sha256']:
                raise RuntimeError('Frame differs from the captured run: ' + shot['file'])
            used[shot['file']] = shot['sha256']
            title = text(f'title-{index}.txt', scene['title'])
            caption = text(f'caption-{index}.txt', scene['caption'])
            inputs = ['-loop', '1', '-framerate', '24', '-i', str(image)]
            # Keep the page's own step bar visible: every label lives in the bottom band.
            # Same palette as the page: an ink band, white text, Zcash gold as the only accent.
            filters = ['scale=1920:1080', 'drawbox=x=0:y=868:w=1920:h=212:color=0x141413@0.95:t=fill',
                       draw(title, 886, 40, face=bold), draw(caption, 942, 28, '0xe9e7df'),
                       draw(identity, 1046, 17, '0xa9a79e'), draw(badge, 1046, 17, '0xf4b728', x='w-tw-80')]
            subtitle = scene['caption']
        else:
            if 'cut' in scene:
                start, end = (shots[name] for name in scene['cut'])
                real = end['elapsed_seconds'] - start['elapsed_seconds']
                title_value = 'Time cut · ' + duration_text(real) + ' of real time'
                body_value = scene['caption']
            else:
                title_value, body_value = scene['title'], scene['body']
            title = text(f'title-{index}.txt', title_value)
            body = text(f'body-{index}.txt', body_value)
            inputs = ['-f', 'lavfi', '-i', 'color=c=0xf6f4ee:s=1920x1080:r=24']
            filters = ['drawbox=x=160:y=290:w=64:h=6:color=0xf4b728:t=fill', draw(badge, 60, 20, '0x8a6408', x='160'),
                       draw(title, 330, 64, '0x141413', x='160', face=bold), draw(body, 450, 34, '0x4a4944', x='160'),
                       draw(identity, 1000, 18, '0x706e66', x='160')]
            subtitle = title_value + ' — ' + body_value.replace('\n', ' ')
        subprocess.run([ffmpeg, '-nostdin', '-hide_banner', '-loglevel', 'error', '-n', *inputs,
                        '-vf', ','.join(filters), '-t', str(seconds), '-an', '-c:v', 'libx264', '-preset', 'fast',
                        '-crf', '20', '-threads', '2', '-pix_fmt', 'yuv420p', '-r', '24', str(clip)],
                       check=True, timeout=180)
        clips.append(clip)
        stamp = lambda v: f'{v // 3600:02}:{v % 3600 // 60:02}:{v % 60:02},000'
        subtitles.append(f'{index + 1}\n{stamp(elapsed)} --> {stamp(elapsed + seconds)}\n{subtitle}\n')
        elapsed += seconds
    listing = text('concat.txt', ''.join(f"file '{escaped(p)}'\n" for p in clips))
    subprocess.run([ffmpeg, '-nostdin', '-hide_banner', '-loglevel', 'error', '-n', '-f', 'concat', '-safe', '0',
                    '-i', str(listing), '-c', 'copy', '-movflags', '+faststart', str(output)], check=True, timeout=120)
    info = json.loads(subprocess.check_output([ffprobe, '-v', 'error', '-show_streams', '-show_format', '-of', 'json', str(output)]))
    if abs(float(info['format']['duration']) - elapsed) > .2 or info['streams'][0]['width'] != 1920:
        raise RuntimeError('Rendered duration/shape differs')
    captions = output.with_suffix('.srt')
    captions.write_text('\n'.join(subtitles))
    return {'passed': True, 'seconds': elapsed, 'scenes': len(clips), 'output': str(output), 'captions': str(captions),
            'auction_id': facts['auction_id'], 'txid': facts['txid'], 'video_sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
            'frames_verified_against_manifest': len(used), 'audio': False, 'published': False}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    # IBM Plex (SIL OFL 1.1), the same files the page serves.
    parser.add_argument('--font', type=pathlib.Path, default=ROOT / 'demo/fonts/ibm-plex-sans-latin-400-normal.woff2')
    parser.add_argument('--bold', type=pathlib.Path, default=ROOT / 'demo/fonts/ibm-plex-sans-latin-600-normal.woff2')
    args = parser.parse_args()
    print(json.dumps(render(args.output.absolute(), args.font.absolute(), args.bold.absolute()), indent=2))


if __name__ == '__main__':
    main()
