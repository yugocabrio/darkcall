#!/usr/bin/env python3
"""Separate pinned NU7 regtest; never changes the existing NU6.3 node/state.

Official Zebra7.0.0-rc.0 matches SDK branch77190ad9. Local Docker RPC only,
2CPU/2GiB, no public peers/funds; persistent data and old journals are preserved.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tarfile
import time
import urllib.request
from regtest_node import ROOT, GENESIS, IMAGE, RpcError, CONFIG as LEGACY_CONFIG

VERSION = '7.0.0-rc.0'
DEPS = ROOT / '.deps/zebra-7.0.0-rc.0-linux-arm64'
DATA = ROOT / '.local/zebra-regtest-nu7-700rc0'
NAME = 'darkcall-zebra-regtest-nu7-700rc0'
ARCHIVE = 'zebrad-7.0.0-rc.0-aarch64-unknown-linux-gnu.tar.gz'
SHA256 = '59adbb7d3e19c0fb7370eeca628787973ee347e421700a36e3b8dbde5c0b86c4'
URL = 'https://github.com/ZcashFoundation/zebra/releases/download/v7.0.0-rc.0/' + ARCHIVE
BRANCH = '77190ad9'
RPC = 'http://127.0.0.1:29342'
CONFIG = LEGACY_CONFIG.replace('"NU6.3" = 2', '"NU6.3" = 2\nNU7 = 2')


def rpc(method, params=None, timeout=20):
    request = urllib.request.Request(RPC, data=json.dumps({
        'jsonrpc':'2.0', 'method':method, 'params':params or [], 'id':'darkcall-nu7-regtest'
    }).encode(), headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        value = json.load(response)
    if value.get('error'): raise RpcError(value['error'])
    return value['result']


def status():
    info = rpc('getblockchaininfo')
    if info['chain'] not in ('test', 'regtest') or rpc('getblockhash', [0]) != GENESIS:
        raise RuntimeError('NU7 RPC is not the isolated regtest genesis')
    upgrade = info['upgrades'].get(BRANCH)
    if upgrade is None or upgrade['activationheight'] != 2:
        raise RuntimeError('NU7 regtest activation differs from pinned SDK branch')
    if info['blocks'] >= 2 and info['consensus']['chaintip'] != BRANCH:
        raise RuntimeError('NU7 regtest tip branch differs from pinned SDK')
    return {'scope':'separate isolated NU7 regtest; not public testnet', 'genesis':GENESIS,
        'blocks':info['blocks'], 'tip':info['bestblockhash'], 'consensus':info['consensus'],
        'nu7_height':2, 'rpc':RPC, 'zebra_version':VERSION}


def inspect():
    result = subprocess.run(['docker','container','inspect',NAME],capture_output=True,text=True)
    if result.returncode: return None
    item = json.loads(result.stdout)[0]
    if item['Config']['Labels'].get('darkcall.scope') != 'isolated-nu7-regtest':
        raise RuntimeError('NU7 container belongs to another task')
    return item


def prepare_binary():
    DEPS.mkdir(parents=True, exist_ok=True)
    archive = DEPS/ARCHIVE
    if not archive.exists(): urllib.request.urlretrieve(URL, archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise RuntimeError('Official pinned NU7 Zebra archive digest differs')
    with tarfile.open(archive) as source:
        member = source.extractfile('zebrad')
        if member is None: raise RuntimeError('Pinned NU7 archive has no Zebra binary')
        binary = member.read()
    target = DEPS/'zebrad'
    if not target.exists():
        with target.open('xb') as out: out.write(binary)
        target.chmod(0o755)
    if hashlib.sha256(target.read_bytes()).digest() != hashlib.sha256(binary).digest():
        raise RuntimeError('Existing NU7 Zebra differs from its pinned archive')


def start():
    prepare_binary()
    DATA.mkdir(mode=0o700, parents=True, exist_ok=True)
    if DATA.is_symlink() or DATA.stat().st_mode & 0o077:
        raise RuntimeError('Private regular NU7 state directory required')
    config = DATA/'zebra.toml'
    if config.exists() and config.read_text() != CONFIG:
        raise RuntimeError('Existing NU7 regtest config changed; preserve its state')
    if not config.exists():
        with config.open('x') as handle: handle.write(CONFIG)
        config.chmod(0o600)
    state = DATA/'state'
    state.mkdir(mode=0o700, exist_ok=True)
    if state.is_symlink(): raise RuntimeError('NU7 state symlink refused')
    container = inspect()
    if container is None:
        subprocess.run(['docker','run','--detach','--name',NAME,
            '--label','darkcall.scope=isolated-nu7-regtest','--platform','linux/arm64',
            '--cpus','2','--memory','2g','--memory-swap','2g','--pids-limit','256',
            '--publish','127.0.0.1:29342:19332',
            '--env','RAYON_NUM_THREADS=2','--env','TOKIO_WORKER_THREADS=2',
            '--mount',f'type=bind,source={DEPS},target=/zebra,readonly',
            '--mount',f'type=bind,source={config},target=/config/zebra.toml,readonly',
            '--mount',f'type=bind,source={state},target=/state',IMAGE,
            '/zebra/zebrad','-c','/config/zebra.toml','--filters','warn','start'],
            check=True,stdout=subprocess.DEVNULL)
    elif not container['State']['Running']:
        subprocess.run(['docker','start',NAME],check=True,stdout=subprocess.DEVNULL)
    deadline = time.monotonic()+45
    while time.monotonic() < deadline:
        try: return status()
        except Exception:
            current = inspect()
            if current and not current['State']['Running']:
                raise RuntimeError('NU7 regtest exited; inspect its retained Docker log')
            time.sleep(0.5)
    raise RuntimeError('NU7 regtest not ready; preserve config/state and inspect its log')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action',required=True)
    for action in ['prepare','start','status','stop']: sub.add_parser(action)
    mine = sub.add_parser('mine')
    mine.add_argument('blocks',type=int)
    args = parser.parse_args()
    if args.action == 'prepare':
        prepare_binary(); result = {'pinned_official_binary_verified':True,'sha256':SHA256,'version':VERSION}
    elif args.action == 'start': result = start()
    elif args.action == 'stop':
        if inspect() is not None:
            subprocess.run(['docker','stop',NAME],check=True,stdout=subprocess.DEVNULL)
        result = {'stopped':True,'data_deleted':False,'old_regtest_changed':False}
    elif args.action == 'mine':
        status()
        if not 1 <= args.blocks <= 200: raise RuntimeError('Local mining supports1–200 blocks')
        hashes = rpc('generate',[args.blocks],timeout=300)
        result = {'generated_blocks':len(hashes),**status()}
    else: result = status()
    print(json.dumps(result,indent=2))


if __name__ == '__main__': main()
