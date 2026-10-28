#!/usr/bin/env python3
"""Render the silent submission video from real-time recordings of the live site.

Recordings come from tools/demo-capture/record.mjs (modes intro, run and
settled): JPEG frames with the browser's own timestamps plus marks at every
action and result. A scene plays one span between two marks at one speed; a
span sped up by 15% or more carries a badge with the real time it covers. Each
frame keeps its own time, so a burst of frames under proving load stays in real
time. Labels are drawn by headless Brave with IBM Plex (labels.mjs). The track
is silent: the author records the narration from the script written beside the
video, and the subtitles follow the same lines.
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
STORY = ROOT / 'demo/video-storyboard-final.json'
CAPTURE = ROOT / 'tools/demo-capture'
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
    for tool in ('ffmpeg', 'ffprobe', 'node'):
        if not shutil.which(tool):
            raise RuntimeError(tool + ' is missing')
    story = json.loads(STORY.read_text())
    if story['format'] != 4:
        raise RuntimeError('Storyboard is not format 4')
    output = args.output.absolute()
    local = ROOT / '.local'
    if output.parent.resolve() != local.resolve() or output.exists():
        raise RuntimeError('Use a fresh output directly under ignored .local')
    work = local / ('demo-video-final-' + secrets.token_hex(5)); work.mkdir(mode=0o700)
    recordings = {name: load_recording(getattr(args, name)) for name in ('intro', 'run', 'settled')}

    run('node', CAPTURE / 'slides.mjs', work / 'slides', cwd=ROOT, timeout=600, capture_output=True)
    slides = {p.stem: p for p in (work / 'slides').glob('*.png')}

    # Pass 1: voice, timing and labels for every scene.
    scenes, labels, elapsed = [], [], 0.0
    for index, scene in enumerate(story['scenes']):
        item = {'index': index, 'narration': scene.get('narration', ''), 'band': scene.get('band')}
        # Time each line at about 150 words a minute, for the author's own reading.
        item['speech_seconds'] = len(item['narration'].split()) / 2.5 if item['narration'] else 0
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
            if speed >= 1.15 and scene.get('badge', True):
                badge = (f'{speed:.0f}×' if speed >= 10 else f'{speed:.1f}×') + f' · {real:.0f} s of real time'
        else:
            item.update(source='slide', slide=scene['slide'], image=slides[scene['slide']])
        item['badge'] = badge
        item['started_at'] = round(elapsed, 3)
        elapsed += item['seconds']
        if 'slide' not in scene:
            item['overlay'] = work / f'label-{index:02}.png'
            # No band on the picture: the subtitles carry the words, so the band names the scene in the script only.
            labels.append({'file': str(item['overlay']), 'chip': story['chips'][scene['chip']], 'badge': badge, 'band': None})
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
        else:
            record, folder, _ = recordings[item['source']]
            listing = work / f'scene-{index:02}.txt'
            item['frames_used'] = clip_list(record, folder, item['start'], item['end'], item['speed'], listing)
            ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-i', item['overlay'], '-filter_complex',
                   f'[0:v]fps={FPS},scale=1920:1080:flags=lanczos:in_range=pc:out_range=tv,setsar=1,tpad=stop_mode=clone:stop_duration=30[b];'
                   f'[b][1:v]overlay=0:0,format=yuv420p{tail}',
                   '-t', f'{seconds:.3f}', *encode, out)
        if abs(duration(out) - seconds) > 1.5 / FPS:
            raise RuntimeError(f'Scene {index} rendered {duration(out):.3f} s, expected {seconds:.3f} s')
        item['clip'] = out

    listing = work / 'video.txt'
    listing.write_text(''.join(f"file '{s['clip']}'\n" for s in scenes))
    ffmpeg('-f', 'concat', '-safe', '0', '-i', listing, '-c', 'copy', '-movflags', '+faststart', output)
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
    captions = output.parent / 'darkcall-submission.srt'
    captions.write_text('\n'.join(f'{n}\n{stamp(a)} --> {stamp(b)}\n{text}\n' for n, (a, b, text) in enumerate(cues, 1)))

    script = ['# DarkCall submission video: narration', '',
              f'Video length {total:.1f} s, silent. Times are video times; read each line inside its window '
              f'(about 150 words a minute). {sum(len(i["narration"].split()) for i in scenes)} words.', '']
    for item in scenes:
        if item['narration']:
            what = item['band'] or ('slide ' + item['slide'] if item.get('slide') else item['source'])
            script += [f"## {stamp(item['started_at'])[3:8]}–{stamp(item['started_at'] + item['seconds'])[3:8]} · {what}", '',
                       item['narration'], '']
    script += ['## Saying the names', '',
               'REP3: "rep three" (レップ・スリー) · coNoir: "co-nwahr" (コー・ノワール) · Halo2: "halo two" (ヘイロー・ツー) ·',
               'FROST (フロスト) · zatoshi (ザトーシ) · Zcash (ジーキャッシュ)', '']
    narration = output.parent / 'darkcall-submission-narration.md'
    narration.write_text('\n'.join(script))

    # Names only, no local paths: this record is committed beside the video.
    report = {'format': 4, 'passed': True, 'seconds': round(total, 2), 'output': output.name, 'captions': captions.name,
              'narration_script': narration.name, 'voice': None,
              'video_sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
              'recordings': {name: {'scope': story['recordings'][name], 'started_at': rec[0]['started_at'],
                                    'frames': len(rec[0]['frames']), 'seconds': rec[0]['end'],
                                    'marks': rec[0]['marks'], 'results': rec[0]['results']}
                             for name, rec in recordings.items()},
              'scenes': [{k: (str(v) if isinstance(v, pathlib.Path) else v) for k, v in item.items()
                          if k not in ('clip', 'overlay', 'image')} for item in scenes]}
    (output.parent / (output.stem + '-manifest.json')).write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    return report


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('intro', 'run', 'settled'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True, help=f'record.mjs {name} output prefix')
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = render(args)
    print(json.dumps({k: v for k, v in report.items() if k in ('passed', 'seconds', 'output', 'voice', 'video_sha256')}, indent=2))


if __name__ == '__main__':
    main()
