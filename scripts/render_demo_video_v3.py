#!/usr/bin/env python3
"""Render the v3 demo video: slides, two real-time screen recordings and the Oct 19 testnet frames.

Recordings come from tools/demo-capture/record.mjs: JPEG frames with the
browser's own timestamps plus marks at every action and result. A scene plays
one span between two marks at one speed, chosen to fit its narration; a span
sped up by 15% or more carries a badge with the real time it covers. Each frame
keeps its own time, so a burst of frames under proving load stays in real time.
Oct 19 frames are checked against the v2 capture manifest by SHA-256. Labels
are drawn by headless Brave with IBM Plex (tools/demo-capture/labels.mjs).
The voice is macOS `say` (Samantha, Enhanced); --no-voice leaves the track
silent for a recorded narration, and narration.md gives its script and times.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import secrets
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
STORY = ROOT / 'demo/video-storyboard-v3.json'
STILLS = ROOT / 'demo/media/v2'
CAPTURE = ROOT / 'tools/demo-capture'
VOICE = 'Samantha (Enhanced)'
FPS = 30
LEAD, TAIL = 0.4, 0.7  # silence before and after each narration line


def run(*command, **kwargs):
    return subprocess.run([str(c) for c in command], check=True, **kwargs)


def ffmpeg(*args):
    run('ffmpeg', '-nostdin', '-hide_banner', '-loglevel', 'error', '-y', *args, timeout=900)


def duration(path):
    return float(subprocess.check_output(['ffprobe', '-v', 'error', '-show_entries', 'format=duration',
                                          '-of', 'csv=p=0', str(path)]))


def frames(seconds):
    return max(1, round(seconds * FPS))


def stamp(seconds):
    ms = round(seconds * 1000)
    return f'{ms // 3600000:02}:{ms // 60000 % 60:02}:{ms // 1000 % 60:02},{ms % 1000:03}'


def load_recording(prefix):
    record = json.loads(pathlib.Path(str(prefix) + '.json').read_text())
    if record.get('format') != 1 or not record.get('completed') or not record.get('frames'):
        raise RuntimeError('Recording is not complete: ' + str(prefix))
    folder = pathlib.Path(str(prefix) + '-frames').resolve()
    marks = {}
    for item in record['marks']:
        marks.setdefault(item['label'], item['t'])
    return record, folder, marks


def at(marks, value):
    label, offset = (value, 0) if isinstance(value, str) else value
    return marks[label] + offset


def speech(text, path, rate):
    run('say', '-v', VOICE, '-r', str(rate), '-o', path.with_suffix('.aiff'), text, timeout=120)
    seconds = duration(path.with_suffix('.aiff'))
    if seconds < 0.25 * len(text.split()):  # `say` can write an almost empty file
        raise RuntimeError('Speech synthesis returned too little audio for: ' + text[:60])
    ffmpeg('-i', path.with_suffix('.aiff'), '-ar', '48000', '-ac', '1', path)
    return duration(path)


def clip_list(record, folder, start, end, speed, listing):
    """Concat-demuxer list: every frame inside [start, end), each for its own real interval / speed."""
    shots = record['frames']
    first = max([i for i, f in enumerate(shots) if f['t'] <= start] or [0])
    chosen = [f for f in shots[first:] if f['t'] < end]
    lines = []
    for index, shot in enumerate(chosen):
        begin = max(shot['t'], start)
        until = chosen[index + 1]['t'] if index + 1 < len(chosen) else end
        lines.append(f"file '{folder / shot['file']}'\nduration {max(until - begin, 0.001) / speed:.4f}\n")
    lines.append(f"file '{folder / chosen[-1]['file']}'\n")
    listing.write_text(''.join(lines))
    return len(chosen)


def render(args):
    for tool in ('ffmpeg', 'ffprobe', 'node') + (() if args.no_voice else ('say',)):
        if not shutil.which(tool):
            raise RuntimeError(tool + ' is missing')
    story = json.loads(STORY.read_text())
    if story['format'] != 3:
        raise RuntimeError('Storyboard is not v3')
    output = args.output.absolute()
    local = ROOT / '.local'
    if output.parent.resolve() != local.resolve() or output.exists():
        raise RuntimeError('Use a fresh output directly under ignored .local')
    work = local / ('demo-video-v3-' + secrets.token_hex(5)); work.mkdir(mode=0o700)
    recordings = {'live': load_recording(args.live), 'settled': load_recording(args.settled)}
    manifest = json.loads((STILLS / 'manifest.json').read_text())
    stills = {shot['name']: shot for shot in manifest['shots']}

    run('node', CAPTURE / 'slides.mjs', work / 'slides', cwd=ROOT, timeout=600, capture_output=True)
    slides = {p.stem: p for p in (work / 'slides').glob('*.png')}

    # Pass 1: voice, timing and labels for every scene.
    scenes, labels, elapsed = [], [], 0.0
    for index, scene in enumerate(story['scenes']):
        item = {'index': index, 'narration': scene.get('narration', ''), 'band': scene.get('band')}
        voice = work / f'voice-{index:02}.wav'
        if item['narration'] and not args.no_voice:
            item['voice'] = voice
            item['speech_seconds'] = speech(scene.get('say', item['narration']), voice, args.rate)
        else:
            # Without a voice, time each line at about 165 words a minute.
            item['speech_seconds'] = len(item['narration'].split()) / 2.75 if item['narration'] else 0
        need = item['speech_seconds'] + LEAD + TAIL if item['narration'] else 0
        if 'slide' in scene:
            need += 0.4
        seconds = scene.get('seconds') or max(need, scene.get('min_seconds', 0))
        item['seconds'] = frames(seconds) / FPS
        badge = None
        if 'clip' in scene:
            record, folder, marks = recordings[scene['clip']]
            start, end = at(marks, scene['from']), at(marks, scene['to'])
            if not 0 <= start < end <= record['end'] + 0.01:
                raise RuntimeError(f'Scene {index}: span outside its recording')
            real = end - start
            speed = max(1.0, real / item['seconds'])
            item.update(source=scene['clip'], start=start, end=end, real_seconds=round(real, 1), speed=round(speed, 2))
            if speed >= 1.15:
                badge = (f'{speed:.0f}×' if speed >= 10 else f'{speed:.1f}×') + f' · {real:.0f} s of real time'
        elif 'stills' in scene:
            files = []
            for name in scene['stills']:
                image = STILLS / stills[name]['file']
                if hashlib.sha256(image.read_bytes()).hexdigest() != stills[name]['sha256']:
                    raise RuntimeError('Frame differs from the captured run: ' + image.name)
                files.append(image)
            item.update(source='oct19-stills', stills=[f.name for f in files], files=files)
        else:
            item.update(source='slide', slide=scene['slide'], image=slides[scene['slide']])
        item['badge'] = badge
        item['started_at'] = round(elapsed, 3)
        elapsed += item['seconds']
        if 'slide' not in scene:
            item['overlay'] = work / f'label-{index:02}.png'
            labels.append({'file': str(item['overlay']), 'chip': story['chips'][scene['chip']], 'badge': badge, 'band': scene.get('band')})
        scenes.append(item)
    plan = work / 'labels.json'
    plan.write_text(json.dumps(labels))
    run('node', CAPTURE / 'labels.mjs', plan, cwd=ROOT, timeout=600, capture_output=True)

    # Pass 2: one video clip and one audio clip per scene.
    # JPEG frames are full range; convert so every scene is tagged and stored as TV-range yuv420p.
    encode = ['-an', '-c:v', 'libx264', '-preset', 'medium', '-crf', '22', '-pix_fmt', 'yuv420p', '-color_range', 'tv', '-r', str(FPS)]
    last = len(scenes) - 1
    for item in scenes:
        index, seconds = item['index'], item['seconds']
        out = work / f'scene-{index:02}.mp4'
        fades = []
        if index == 0:
            fades.append('fade=t=in:st=0:d=0.5')
        if index == last:
            fades.append(f'fade=t=out:st={seconds - 0.7:.3f}:d=0.7')
        tail = (',' + ','.join(fades)) if fades else ''
        if item['source'] == 'slide':
            ffmpeg('-loop', '1', '-framerate', str(FPS), '-i', item['image'], '-vf', 'format=yuv420p' + tail,
                   '-t', f'{seconds:.3f}', *encode, out)
        elif item['source'] == 'oct19-stills':
            parts = []
            total, count = frames(seconds), len(item['files'])
            for number, image in enumerate(item['files']):
                part = work / f'scene-{index:02}-{number}.mp4'
                n = total // count + (number < total % count)
                # Upscale first so the slow push-in moves smoothly.
                ffmpeg('-loop', '1', '-framerate', str(FPS), '-i', image, '-i', item['overlay'], '-filter_complex',
                       f"[0:v]scale=3840:2160:in_range=pc:out_range=tv,zoompan=z='1+0.05*on/{n}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
                       f":d=1:s=1920x1080:fps={FPS}[b];[b][1:v]overlay=0:0,format=yuv420p",
                       '-frames:v', str(n), *encode, part)
                parts.append(part)
            listing = work / f'scene-{index:02}.txt'
            listing.write_text(''.join(f"file '{p}'\n" for p in parts))
            ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-c', 'copy', out)
        else:
            record, folder, _ = recordings[item['source']]
            listing = work / f'scene-{index:02}.txt'
            item['frames_used'] = clip_list(record, folder, item['start'], item['end'], item['speed'], listing)
            ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-i', item['overlay'], '-filter_complex',
                   f'[0:v]fps={FPS},scale=1920:1080:in_range=pc:out_range=tv,setsar=1,tpad=stop_mode=clone:stop_duration=2[b];'
                   f'[b][1:v]overlay=0:0,format=yuv420p{tail}',
                   '-t', f'{seconds:.3f}', *encode, out)
        if abs(duration(out) - seconds) > 1.5 / FPS:
            raise RuntimeError(f'Scene {index} rendered {duration(out):.3f} s, expected {seconds:.3f} s')
        sound = work / f'sound-{index:02}.wav'
        if 'voice' in item:
            ffmpeg('-i', item['voice'], '-af', f'adelay={int(LEAD * 1000)},apad', '-t', f'{seconds:.3f}',
                   '-ar', '48000', '-ac', '1', sound)
        else:
            ffmpeg('-f', 'lavfi', '-i', 'anullsrc=r=48000:cl=mono', '-t', f'{seconds:.3f}', sound)
        item['clip'], item['sound'] = out, sound

    listing = work / 'video.txt'
    listing.write_text(''.join(f"file '{s['clip']}'\n" for s in scenes))
    picture = work / 'picture.mp4'
    ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-c', 'copy', picture)
    listing = work / 'sound.txt'
    listing.write_text(''.join(f"file '{s['sound']}'\n" for s in scenes))
    ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-af', 'loudnorm=I=-16:TP=-1.5:LRA=11', '-ar', '48000',
           '-c:a', 'aac', '-b:a', '128k', work / 'sound.m4a')
    ffmpeg('-i', picture, '-i', work / 'sound.m4a', '-map', '0:v', '-map', '1:a', '-c', 'copy', '-shortest',
           '-movflags', '+faststart', output)
    total = duration(output)
    if abs(total - elapsed) > 0.25:
        raise RuntimeError(f'Rendered {total:.2f} s, expected {elapsed:.2f} s')

    # Captions: each narration line, split at sentences, timed by length.
    cues = []
    for item in scenes:
        if not item['narration']:
            continue
        begin = item['started_at'] + LEAD
        parts = [p for p in re.split(r'(?<=[.:])\s+', item['narration']) if p]
        weights = [len(p) for p in parts]
        for part, weight in zip(parts, weights):
            length = item['speech_seconds'] * weight / sum(weights)
            cues.append((begin, begin + length, part))
            begin += length
    captions = output.with_suffix('.srt')
    captions.write_text('\n'.join(f'{n}\n{stamp(a)} --> {stamp(b)}\n{text}\n' for n, (a, b, text) in enumerate(cues, 1)))

    script = ['# DarkCall demo video v3: narration', '',
              f'Video length {total:.1f} s. Times are video times; read each line inside its window.', '']
    for item in scenes:
        if item['narration']:
            what = item['band'] or 'slide ' + item['slide']
            script += [f"## {stamp(item['started_at'])[3:8]}–{stamp(item['started_at'] + item['seconds'])[3:8]} · {what}", '',
                       item['narration'], '']
    script += ['## Saying the names', '',
               'REP3: "rep three" (レップ・スリー) · coNoir: "co-nwahr" (コー・ノワール) · Halo2: "halo two" (ヘイロー・ツー) ·',
               'FROST (フロスト) · zatoshi (ザトーシ) · Zcash (ジーキャッシュ)', '']
    narration = output.parent / (output.stem + '-narration.md')
    narration.write_text('\n'.join(script))

    # Names only, no local paths: this record is committed beside the video.
    report = {'format': 3, 'passed': True, 'seconds': round(total, 2), 'output': output.name, 'captions': captions.name,
              'narration_script': narration.name, 'voice': None if args.no_voice else f'macOS say, {VOICE}, rate {args.rate}',
              'video_sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
              'recordings': {name: {'scope': story['recordings'][name], 'started_at': rec[0]['started_at'],
                                    'frames': len(rec[0]['frames']), 'seconds': rec[0]['end'],
                                    'marks': rec[0]['marks'], 'results': rec[0]['results']}
                             for name, rec in recordings.items()},
              'scenes': [{k: (str(v) if isinstance(v, pathlib.Path) else v) for k, v in item.items()
                          if k not in ('clip', 'sound', 'voice', 'overlay', 'files', 'image')} for item in scenes]}
    (output.parent / (output.stem + '-manifest.json')).write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    return report


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--live', type=pathlib.Path, required=True, help='record.mjs live output prefix')
    parser.add_argument('--settled', type=pathlib.Path, required=True, help='record.mjs settled output prefix')
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--rate', type=int, default=175)
    parser.add_argument('--no-voice', action='store_true')
    args = parser.parse_args()
    report = render(args)
    print(json.dumps({k: v for k, v in report.items() if k in ('passed', 'seconds', 'output', 'voice', 'video_sha256')}, indent=2))


if __name__ == '__main__':
    main()
