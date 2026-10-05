#!/usr/bin/env python3
"""Small isolated Zebra 6.3.0 regtest. No public network or mainnet interaction.

Pinned official ARM Linux binary runs with 2 CPUs / 2 GiB in Docker. Host RPC
binds only 127.0.0.1. Persistent data stays ignored; never delete/reset it here.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tarfile
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEPS = ROOT / '.deps/zebra-6.3.0-linux-arm64'
DATA = ROOT / '.local/zebra-regtest-630'
NAME = 'darkcall-zebra-regtest-630'
IMAGE = 'ubuntu@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55'
ARCHIVE = 'zebrad-6.3.0-aarch64-unknown-linux-gnu.tar.gz'
SHA256 = '807db2dce0692b3fe237e7e470dabab952af0e30bb9ea2549d43c5ad2da77360'
URL = 'https://github.com/ZcashFoundation/zebra/releases/download/v6.3.0/' + ARCHIVE
GENESIS = '029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327'
BRANCH = '37a5165b'
RPC = 'http://127.0.0.1:29332'
LABEL = 'darkcall.scope=isolated-regtest'
CONFIG = '''[network]
network = "Regtest"
listen_addr = "127.0.0.1:18233"

[network.testnet_parameters.activation_heights]
Overwinter = 1
Sapling = 1
Blossom = 1
Heartwood = 1
Canopy = 1
NU5 = 2
NU6 = 2
"NU6.1" = 2
"NU6.2" = 2
"NU6.3" = 2

[rpc]
listen_addr = "0.0.0.0:19332"
enable_cookie_auth = false

[state]
cache_dir = "/state"

[mining]
# Public Zebra documentation's disposable test miner; no key is stored here.
miner_address = "t27eWDgjFYJGVXmzrXeVjnb5J3uXDM9xH9v"
'''


def rpc(method, params=None, timeout=20):
    request = urllib.request.Request(RPC, data=json.dumps({
        'jsonrpc': '2.0', 'method': method, 'params': params or [], 'id': 'darkcall-regtest'
    }).encode(), headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        result = json.load(response)
    if result.get('error'):
        raise RuntimeError('regtest RPC: ' + str(result['error']))
    return result['result']


def status():
    info = rpc('getblockchaininfo')
    genesis = rpc('getblockhash', [0])
    if info['chain'] not in ('test', 'regtest') or genesis != GENESIS:
        raise RuntimeError('local RPC is not the pinned isolated regtest genesis')
    upgrade = info['upgrades'].get(BRANCH)
    if upgrade is None or upgrade['activationheight'] != 2:
        raise RuntimeError('local regtest NU6.3 activation differs from pinned configuration')
    if info['blocks'] >= 2 and info['consensus']['chaintip'] != BRANCH:
        raise RuntimeError('local regtest branch differs from pinned native SDK')
    return {'scope': 'isolated Zebra regtest; not standard testnet', 'genesis': genesis,
            'blocks': info['blocks'], 'tip': info['bestblockhash'], 'consensus': info['consensus'],
            'nu6_3_height': 2, 'rpc': RPC, 'zebra_version': '6.3.0'}


def inspect():
    result = subprocess.run(['docker', 'container', 'inspect', NAME], capture_output=True, text=True)
    if result.returncode:
        return None
    item = json.loads(result.stdout)[0]
    if item['Config']['Labels'].get('darkcall.scope') != 'isolated-regtest':
        raise RuntimeError('container name belongs to a different task; refusing to touch it')
    return item


def prepare_binary():
    DEPS.mkdir(parents=True, exist_ok=True)
    archive = DEPS / ARCHIVE
    if not archive.exists():
        urllib.request.urlretrieve(URL, archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise RuntimeError('pinned official Zebra archive hash differs')
    with tarfile.open(archive) as tf:
        binary = tf.extractfile('zebrad').read()
        target = DEPS / 'zebrad'
        if not target.exists():
            target.write_bytes(binary)
            target.chmod(0o755)
        if hashlib.sha256(target.read_bytes()).digest() != hashlib.sha256(binary).digest():
            raise RuntimeError('extracted Zebra binary differs from pinned archive')


def start():
    prepare_binary()
    DATA.mkdir(mode=0o700, parents=True, exist_ok=True)
    if DATA.is_symlink() or (DATA.stat().st_mode & 0o077):
        raise RuntimeError('regtest directory must be private and not a symlink')
    config = DATA / 'zebra.toml'
    if config.exists() and config.read_text() != CONFIG:
        raise RuntimeError('regtest configuration changed; explicit review is required')
    if not config.exists():
        with config.open('x') as handle:
            handle.write(CONFIG)
        config.chmod(0o600)
    state = DATA / 'state'
    state.mkdir(mode=0o700, exist_ok=True)
    container = inspect()
    if container is None:
        subprocess.run(['docker', 'run', '--detach', '--name', NAME, '--label', LABEL,
                        '--platform', 'linux/arm64', '--cpus', '2', '--memory', '2g',
                        '--memory-swap', '2g', '--pids-limit', '256',
                        '--publish', '127.0.0.1:29332:19332',
                        '--env', 'RAYON_NUM_THREADS=2', '--env', 'TOKIO_WORKER_THREADS=2',
                        '--mount', f'type=bind,source={DEPS},target=/zebra,readonly',
                        '--mount', f'type=bind,source={config},target=/config/zebra.toml,readonly',
                        '--mount', f'type=bind,source={state},target=/state', IMAGE,
                        '/zebra/zebrad', '-c', '/config/zebra.toml', '--filters', 'warn', 'start'],
                       check=True, stdout=subprocess.DEVNULL)
    elif not container['State']['Running']:
        subprocess.run(['docker', 'start', NAME], check=True, stdout=subprocess.DEVNULL)
    deadline = time.monotonic() + 45
    last_error = None
    while time.monotonic() < deadline:
        try:
            return status()
        except Exception as error:
            last_error = error
            container = inspect()
            if container and not container['State']['Running']:
                raise RuntimeError('regtest exited; inspect docker logs for ' + NAME) from error
            time.sleep(0.5)
    raise RuntimeError('regtest startup did not become ready: ' + str(last_error))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    sub.add_parser('start')
    sub.add_parser('status')
    sub.add_parser('stop')
    mine = sub.add_parser('mine')
    mine.add_argument('blocks', type=int)
    mine.add_argument('--address')
    args = parser.parse_args()
    if args.action == 'start':
        result = start()
    elif args.action == 'stop':
        if inspect() is not None:
            subprocess.run(['docker', 'stop', NAME], check=True, stdout=subprocess.DEVNULL)
        result = {'stopped': True, 'data_deleted': False}
    elif args.action == 'mine':
        status()  # Genesis and upgrade guard BEFORE the mutation.
        if not 1 <= args.blocks <= 200:
            raise RuntimeError('local mining gate supports 1..200 blocks per call')
        hashes = rpc('generatetoaddress', [args.blocks, args.address], timeout=300) if args.address else rpc('generate', [args.blocks], timeout=300)
        result = {'generated_blocks': len(hashes), **status()}
    else:
        result = status()
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
