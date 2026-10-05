#!/usr/bin/env python3
"""Render an explicitly edited, captioned demo draft from verified UI stills.

No browser automation, secret reads, proof generation, uploads or voice synthesis.
Images are captured with the native browser tool after actual verification.
"""
import argparse
import hashlib
import json
import os
import pathlib
import secrets
import shutil
import subprocess
from review_kit import ROOT, TXID


def escaped(path):
    return str(path).replace('\\','\\\\').replace(':','\\:').replace("'","'\\''")


def render(output,font):
    ffmpeg=shutil.which('ffmpeg'); ffprobe=shutil.which('ffprobe')
    if not ffmpeg or not ffprobe or not font.is_file(): raise RuntimeError('FFmpeg/font prerequisites missing')
    story=json.loads((ROOT/'demo/video-storyboard.json').read_text())
    if story['format']!=1 or story['txid']!=TXID: raise RuntimeError('Storyboard differs from canonical run')
    local=ROOT/'.local'
    if local.is_symlink(): raise RuntimeError('Local output symlink refused')
    local.mkdir(mode=0o700,exist_ok=True)
    if output.parent.resolve()!=local.resolve() or output.exists():
        raise RuntimeError('Use a fresh output directly under ignored .local')
    target=local/('demo-video-build-'+secrets.token_hex(6)); target.mkdir(mode=0o700)
    clips=[]; images={}; elapsed=0; subtitles=[]
    def text(name,value):
        path=target/name; path.write_text(value); return path
    label=text('label.txt','DARKCALL / ZECATHON  |  EDITED PROOF-BACKED DEMO DRAFT')
    def draw(path,y,size,color='0xd2eca5',x='120'):
        return f"drawtext=fontfile='{escaped(font)}':textfile='{escaped(path)}':expansion=none:fontsize={size}:fontcolor={color}:x={x}:y={y}:line_spacing=12"
    for index,scene in enumerate(story['scenes']):
        scope=scene.get('execution','recorded')
        if scope not in ('entry','recorded'): raise RuntimeError('Unknown scene execution scope')
        identity=text(f'identity-{index}.txt',
            'FRESH LOCAL AUCTION  |  SYNTHETIC DEPOSITS  |  NO NATIVE PAYMENT SENT' if scope=='entry'
            else 'RECORDED FUNDED REGTEST TX  '+TXID)
        duration=scene['seconds']
        if type(duration)!=int or not 1<=duration<=30: raise RuntimeError('Unbounded scene duration')
        title=text(f'title-{index}.txt',scene['title'])
        caption=text(f'caption-{index}.txt',scene['caption'])
        clip=target/f'scene-{index:02}.mp4'
        if 'image' in scene:
            image=ROOT/'demo/media'/scene['image']
            if image.parent!=ROOT/'demo/media' or not image.is_file(): raise RuntimeError('Unknown screenshot')
            images[scene['image']]=hashlib.sha256(image.read_bytes()).hexdigest()
            inputs=['-loop','1','-framerate','24','-i',str(image)]
            filters=['scale=1680:760:force_original_aspect_ratio=decrease',
                     'pad=1920:1080:(ow-iw)/2:180:color=0x10160f',draw(label,28,22),draw(title,74,42)]
        else:
            inputs=['-f','lavfi','-i','color=c=0x10160f:s=1920x1080:r=24']
            body=text(f'body-{index}.txt',scene['body'])
            filters=[draw(label,40,22),draw(title,170,60),draw(body,320,36,'0xedeee5')]
        filters.extend([draw(caption,944,28,'0xedeee5'),draw(identity,1040,15,'0x8c9986')])
        command=[ffmpeg,'-nostdin','-hide_banner','-loglevel','error','-n',*inputs,
            '-vf',','.join(filters),'-t',str(duration),'-an','-c:v','libx264','-preset','fast',
            '-crf','22','-threads','2','-pix_fmt','yuv420p','-r','24',str(clip)]
        subprocess.run(command,check=True,timeout=180)
        clips.append(clip)
        def stamp(value): return f'{value//3600:02}:{value%3600//60:02}:{value%60:02},000'
        subtitles.append(f"{index+1}\n{stamp(elapsed)} --> {stamp(elapsed+duration)}\n{scene['caption']}\n")
        elapsed+=duration
    listing=text('concat.txt',''.join(f"file '{escaped(p)}'\n" for p in clips))
    subprocess.run([ffmpeg,'-nostdin','-hide_banner','-loglevel','error','-n','-f','concat','-safe','0',
                    '-i',str(listing),'-c','copy','-movflags','+faststart',str(output)],check=True,timeout=60)
    info=json.loads(subprocess.check_output([ffprobe,'-v','error','-show_streams','-show_format','-of','json',str(output)]))
    video=info['streams'][0]
    if (abs(float(info['format']['duration'])-elapsed)>.1 or video['width']!=1920
            or video['height']!=1080 or len(info['streams'])!=1):
        raise RuntimeError('Rendered duration/shape differs')
    text('captions.srt','\n'.join(subtitles))
    return {'passed':True,'scope':story['scope'],'txid':TXID,'seconds':elapsed,'scenes':len(clips),
        'width':1920,'height':1080,'audio':False,'published':False,'output':str(output),
        'video_bytes':output.stat().st_size,'video_sha256':hashlib.sha256(output.read_bytes()).hexdigest(),
        'image_sha256':images,'private_material_read':False,'proof_generation_in_video':False}


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--output',type=pathlib.Path,required=True)
    parser.add_argument('--font',type=pathlib.Path,default=pathlib.Path('/System/Library/Fonts/Supplemental/Arial.ttf'))
    args=parser.parse_args(); print(json.dumps(render(args.output.absolute(),args.font.absolute()),indent=2))


if __name__=='__main__': main()
