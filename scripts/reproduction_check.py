#!/usr/bin/env python3
"""Read-only public-replay prerequisites; no proof/keys/RPC/network are used.

Passing this inventory is not cryptographic verification or independent-machine
reproduction. Build in the final source directory: Rust helpers embed that path.
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys
from review_kit import ROOT, BUNDLE, AUTHORITY, public_links

BB_PIN='87aa5283d5b0cab6632ed49fdd25f6548d2ba91f'
CO_PIN='467a2abbad940d40ed78f38d424cc21920988976'


def command(*args):
    result=subprocess.run(list(map(str,args)),cwd=ROOT,stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL,timeout=30)
    if result.returncode: raise RuntimeError('Prerequisite command unavailable or failed')
    return result.stdout


def file_record(path):
    path=ROOT/path
    if not path.is_file(): raise RuntimeError('Required public tool/artifact missing')
    return {'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}


def inspect():
    checks=[]
    def check(name,fn):
        try: checks.append({'name':name,'passed':True,'result':fn()})
        except Exception as error:
            # No arbitrary subprocess output, credential URLs or artifact bytes.
            checks.append({'name':name,'passed':False,'error':type(error).__name__,
                           'next':'See docs/reproduce.md; no private runtime input is needed'})
    check('python',lambda: {'version':platform.python_version()} if sys.version_info>=(3,10)
          else (_ for _ in ()).throw(RuntimeError('Python 3.10+ required')))
    def artifacts():
        names=[BUNDLE+'/bundle.json',AUTHORITY]
        manifest=json.loads((ROOT/names[0]).read_bytes())
        names.extend(BUNDLE+'/'+name for name in manifest['artifacts'])
        return public_links({name:(ROOT/name).read_bytes() for name in names})
    check('canonical-public-artifacts',artifacts)
    def compiler():
        version=command(ROOT/'.deps/noir-1.0.0-rc.2/nargo','--version').decode()
        if 'nargo version = 1.0.0-rc.2\n' not in version: raise RuntimeError('Wrong Nargo')
        return {'version':'1.0.0-rc.2'}
    check('pinned-noir',compiler)
    for name in ('settlement_poseidon4','delivery_flags4'):
        check('compiled-'+name,lambda name=name:file_record(f'circuits/{name}/target/darkcall_{name}.json'))
    def bb():
        source=ROOT/'.deps/barretenberg-src'
        if command('git','-C',source,'rev-parse','HEAD').decode().strip()!=BB_PIN:
            raise RuntimeError('Wrong source pin')
        expected=b''.join((ROOT/'patches'/name).read_bytes() for name in
                          ('barretenberg-apple-clang15.patch','barretenberg-macos-ipc.patch'))
        if command('git','-C',source,'diff','--')!=expected: raise RuntimeError('Unknown verifier patch')
        binary=pathlib.Path('.deps/barretenberg-src/barretenberg/cpp/build-darkcall/bin/bb')
        if not os.access(ROOT/binary,os.X_OK): raise RuntimeError('Verifier not executable')
        return {'source':BB_PIN,'exact_known_patches':True,**file_record(binary)}
    check('fixed-cpp-verifier',bb)
    def crs():
        source=ROOT/'.deps/co-snarks'
        if command('git','-C',source,'rev-parse','HEAD').decode().strip()!=CO_PIN:
            raise RuntimeError('Wrong CRS source pin')
        records={}
        for name in ('bn254_g1.dat','bn254_g2.dat','grumpkin_g1.dat'):
            rel='co-noir/co-noir-common/src/crs/'+name
            expected=command('git','-C',source,'rev-parse','HEAD:'+rel).decode().strip()
            actual=command('git','-C',source,'hash-object',rel).decode().strip()
            if expected!=actual: raise RuntimeError('CRS differs from pinned Git blob')
            records[name]={'git_blob':actual,**file_record(pathlib.Path('.deps/co-snarks')/rel)}
        return {'source':CO_PIN,'files':records}
    check('pinned-crs',crs)
    for name in ('target/release/darkcall-native-gate',
                 '.deps/nu7-compat-target/release/darkcall-nu7-compat'):
        def executable(name=name):
            if not os.access(ROOT/name,os.X_OK): raise RuntimeError('Native helper not executable')
            return file_record(name)
        check(name,executable)
    for name in ('Cargo.lock','tools/nu7-compat/Cargo.lock'):
        check(name,lambda name=name:file_record(name))
    return {'scope':'public replay prerequisite inventory; not proof verification',
        'passed':all(c['passed'] for c in checks),'host':{'system':platform.system(),'machine':platform.machine()},
        'free_disk_bytes':shutil.disk_usage(ROOT).free,'checks':checks,
        'proofs_cryptographically_verified':False,'second_machine_reproduction':False,
        'private_runtime_read':False,'network_or_node_used':False,
        'native_binaries_must_be_built_here':True,
        'linux_build_and_parallel_performance_verified':False}


def main():
    parser=argparse.ArgumentParser(description=__doc__); parser.parse_args()
    result=inspect(); print(json.dumps(result,indent=2)); sys.exit(0 if result['passed'] else 1)


if __name__=='__main__': main()
