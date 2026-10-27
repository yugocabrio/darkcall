#!/usr/bin/env python3
"""Package a committed source revision and public artifacts for local review.

No runtime directories or Git configuration are read into the archive. The
manifest detects damage; it is not an authenticity signature or a proof verifier.
This command neither publishes the repository nor submits the project.
"""
import argparse
import gzip
import hashlib
import io
import json
import os
import pathlib
import re
import subprocess
import tarfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
KIND = 'darkcall-source-review-kit-v1'
MAX_TOTAL = 32 * 1024 * 1024
MAX_FILE = 8 * 1024 * 1024
MAX_FILES = 1500
ROOT_FILES = {'.gitignore', 'Cargo.lock', 'Cargo.toml', 'LICENSE', 'Makefile', 'README.md', 'demo.sh', 'verify.sh'}
ROOT_DIRS = {'circuits', 'crates', 'demo', 'docker', 'docs', 'evidence', 'patches', 'scripts', 'tools', 'vendor'}
# Vendored upstream source whose name matches the witness-artifact rule.
SOURCE_EXCEPTIONS = {'vendor/halo2_gadgets/src/ecc/chip/witness_point.rs'}
BUNDLE = 'demo/artifacts/canonical-nu7-funded-v1'
AUTHORITY = 'demo/authority/canonical-nu7-funded-v1.json'
TXID = '47019e02e49557ca1e8ef899b94ff1e464a9f29ec14accc298fb0bad3a211938'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def safe_source(name, data):
    p = pathlib.PurePosixPath(name)
    if (p.is_absolute() or str(p) != name or not p.parts or '..' in p.parts
            or '\\' in name or any(ord(c) < 32 for c in name)
            or (len(p.parts) == 1 and name not in ROOT_FILES)
            or (len(p.parts) > 1 and p.parts[0] not in ROOT_DIRS)):
        raise RuntimeError('Unreviewed source path')
    if (set(p.parts) & {'.git', '.deps', '.local', 'target', '__pycache__'}
            or name.startswith('evidence/private/') or p.name == 'Prover.toml'
            or p.name.startswith(('.env', 'private-'))
            or ('witness' in p.name.lower() and name not in SOURCE_EXCEPTIONS)
            or p.suffix in {'.pczt', '.db', '.db-shm', '.db-wal', '.sqlite', '.sqlite3',
                            '.sqlite-shm', '.sqlite-wal', '.pyc', '.pem', '.key', '.log'}):
        raise RuntimeError('Runtime/private artifact refused')
    # High-confidence credential markers only; this does not claim to be a
    # complete secret scanner. Git metadata and ignored files never enter.
    patterns = [rb'github[_]pat_[A-Za-z0-9_]{20,}', rb'gh[pousr]_[A-Za-z0-9]{30,}',
                rb'-----BEGIN [A-Z ]*PRIVATE KEY-----',
                rb'https?://[^/\s:@]+:[^/\s@]+@']
    if any(re.search(pattern, data) for pattern in patterns):
        raise RuntimeError('Credential marker refused; content is not printed')
    if len(data) > MAX_FILE:
        raise RuntimeError('Oversized source file')


def public_links(files):
    """Check packaging/context links, without pretending to verify proofs."""
    from funded_public_bundle import KIND as BUNDLE_KIND, LIMITS
    from public_bundle import SDK_PIN, HASH_RULE, link_receipt_statements
    manifest = json.loads(files[BUNDLE + '/bundle.json'])
    if (set(manifest) != {'format', 'kind', 'sdk', 'hash_rule', 'artifacts'}
            or manifest['format'] != 1 or manifest['kind'] != BUNDLE_KIND
            or manifest['sdk'] != SDK_PIN or manifest['hash_rule'] != HASH_RULE
            or set(manifest['artifacts']) != set(LIMITS)):
        raise RuntimeError('Canonical public bundle differs')
    actual = {name[len(BUNDLE)+1:] for name in files if name.startswith(BUNDLE+'/')}
    if actual != set(LIMITS) | {'bundle.json'}:
        raise RuntimeError('Unlisted public artifact')
    for name, limit in LIMITS.items():
        data = files[BUNDLE + '/' + name]
        if len(data) > limit or digest(data) != manifest['artifacts'][name]:
            raise RuntimeError('Canonical public artifact damaged')
    if json.loads(files[BUNDLE+'/mpc-result.json']) != {'run':'mpc-public', 'hash_rule':HASH_RULE}:
        raise RuntimeError('External proof path refused')
    identity = json.loads(files[BUNDLE+'/nu7-native-identity.json'])
    if identity['txid'] != TXID or identity['branch'] != '77190ad9':
        raise RuntimeError('Unexpected canonical transaction')
    authority = json.loads(files[AUTHORITY])
    certificate = json.loads(files[BUNDLE+'/final-book-certificate.json'])
    book = json.loads(files[BUNDLE+'/receipt-book.json'])
    endorsements = certificate['endorsements']
    if (len(endorsements)!=3 or {e['operator'] for e in endorsements}!={0,1,2}
            or any(e['book']!=endorsements[0]['book'] for e in endorsements)
            or int.from_bytes(bytes(endorsements[0]['book']['accepted_root']),'big')!=int(book['root'])
            or authority['context']['conditions_hash']!=book['condition_digest']
            or any(authority['context'][k]!=book[k] for k in ('units','reserve','budget','processing_fee'))):
        raise RuntimeError('Separate authority/accepted-book context differs')
    link_receipt_statements(files[BUNDLE+'/receipt-proof-public/public-input0'],
                            files[BUNDLE+'/mpc-public/public-input0'])
    return {'txid':TXID, 'bundle':BUNDLE, 'authority':AUTHORITY,
            'authority_sha256':digest(files[AUTHORITY]),
            'public_bundle_bytes':sum(len(files[BUNDLE+'/'+n]) for n in actual),
            'proofs_cryptographically_verified':False,
            'chain_inclusion_verified':False}


def git_bytes(root, *args):
    # No remotes/config/log contents printed, and no shell interpolation.
    return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.DEVNULL)


def committed_files(root, revision):
    commit = git_bytes(root, 'rev-parse', '--verify', revision+'^{commit}').decode().strip()
    entries = git_bytes(root, 'ls-tree', '-rlz', commit).split(b'\0')
    names = set(); total=0
    for entry in filter(None, entries):
        head, name = entry.split(b'\t', 1)
        mode, kind, _, size = head.split()
        if mode not in {b'100644', b'100755'} or kind != b'blob':
            raise RuntimeError('Links/submodules/non-files refused')
        if int(size)>MAX_FILE: raise RuntimeError('Oversized committed source file')
        total+=int(size)
        if total>MAX_TOTAL or len(names)>=MAX_FILES: raise RuntimeError('Committed source exceeds bound')
        safe_source(name.decode('utf-8'),b'')
        names.add(name.decode('utf-8'))
    data = git_bytes(root, 'archive', '--format=tar', commit)
    if len(data) > MAX_TOTAL: raise RuntimeError('Source archive exceeds bound')
    files = {}; modes = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:') as archive:
        for member in archive:
            if member.isdir(): continue
            if not member.isfile() or member.name in files or member.size > MAX_FILE:
                raise RuntimeError('Unsupported source archive member')
            value = archive.extractfile(member).read(MAX_FILE+1)
            safe_source(member.name, value)
            files[member.name] = value
            modes[member.name] = 0o755 if member.mode & 0o111 else 0o644
    if set(files) != names or len(files) > MAX_FILES or sum(map(len,files.values())) > MAX_TOTAL:
        raise RuntimeError('Source tree/export differs or exceeds bounds')
    return commit, files, modes


def create(root, revision, output):
    commit, source, modes = committed_files(root, revision)
    links = public_links(source)
    if 'docs/judge-review.md' not in source:
        raise RuntimeError('Commit the review entry point before packaging')
    tree = git_bytes(root, 'rev-parse', commit+'^{tree}').decode().strip()
    timestamp = int(git_bytes(root, 'show', '-s', '--format=%ct', commit))
    prefix = 'darkcall-review-'+commit[:12]
    files = {'source/'+k:v for k,v in source.items()}
    files['REVIEW.md'] = source['docs/judge-review.md']
    manifest = {'format':1, 'kind':KIND, 'commit':commit, 'tree':tree,
        'canonical':links, 'source_files':len(source),
        'lock_sha256':{name:digest(source[name]) for name in ('Cargo.lock','tools/nu7-compat/Cargo.lock')},
        'excluded':['Git metadata','ignored runtime/dependencies','private actor identities'],
        'checksum_is_authenticity_proof':False,
        'files':{name:{'sha256':digest(data),'bytes':len(data),
            'mode':modes[name[7:]] if name.startswith('source/') else 0o644}
            for name,data in sorted(files.items())}}
    files['manifest.json'] = (json.dumps(manifest,indent=2,sort_keys=True)+'\n').encode()
    output = pathlib.Path(output).absolute()
    # Keep unpublished archives in ignored state. Never replace an earlier kit.
    local = root/'.local'
    if local.is_symlink(): raise RuntimeError('Local output symlink refused')
    local.mkdir(mode=0o700,exist_ok=True)
    if output.parent.resolve(strict=True) != local.resolve(strict=True):
        raise RuntimeError('Create archive directly in ignored .local')
    fd=os.open(output,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb') as handle:
        with gzip.GzipFile(filename='',mode='wb',fileobj=handle,mtime=0) as compressed:
            with tarfile.open(fileobj=compressed,mode='w|',format=tarfile.USTAR_FORMAT) as archive:
                for name,data in sorted(files.items()):
                    member=tarfile.TarInfo(prefix+'/'+name)
                    member.size=len(data); member.mtime=timestamp
                    member.mode=manifest['files'][name]['mode'] if name!='manifest.json' else 0o644
                    archive.addfile(member,io.BytesIO(data))
        handle.flush(); os.fsync(handle.fileno())
    checked = check(output, expected_commit=commit)
    return {**checked,'archive':str(output),'archive_sha256':digest(output.read_bytes()),
            'archive_bytes':output.stat().st_size,'published':False}


def check(path, expected_commit=None):
    """Bounded read-only audit. Never extract an untrusted tar."""
    files={}; modes={}; prefix=None; total=0
    if pathlib.Path(path).stat().st_size > MAX_TOTAL:
        raise RuntimeError('Compressed archive exceeds bound')
    with tarfile.open(path,mode='r:gz') as archive:
        for member in archive:
            if not member.isfile() or member.size > MAX_FILE or len(files) >= MAX_FILES+2:
                raise RuntimeError('Archive type/count/size refused')
            parts=member.name.split('/',1)
            if len(parts)!=2 or not re.fullmatch(r'darkcall-review-[0-9a-f]{12}',parts[0]):
                raise RuntimeError('Archive root/path refused')
            if prefix is None: prefix=parts[0]
            name=parts[1]
            if parts[0]!=prefix or name in files or member.mode not in (0o644,0o755):
                raise RuntimeError('Archive root/duplicate/mode differs')
            if name not in ('REVIEW.md','manifest.json'):
                if not name.startswith('source/'): raise RuntimeError('Unexpected archive path')
                safe_source(name[7:],b'')
            value=archive.extractfile(member).read(MAX_FILE+1)
            if name.startswith('source/'): safe_source(name[7:],value)
            total+=len(value)
            if total>MAX_TOTAL: raise RuntimeError('Expanded archive exceeds bound')
            files[name]=value; modes[name]=member.mode
    manifest=json.loads(files.pop('manifest.json'))
    if (set(manifest)!={'format','kind','commit','tree','canonical','source_files',
                       'lock_sha256','excluded','checksum_is_authenticity_proof','files'}
            or manifest['format']!=1 or manifest['kind']!=KIND
            or not re.fullmatch(r'[0-9a-f]{40}',manifest['commit'])
            or not re.fullmatch(r'[0-9a-f]{40}',manifest['tree'])
            or prefix!='darkcall-review-'+manifest['commit'][:12]
            or manifest['checksum_is_authenticity_proof'] is not False
            or (expected_commit is not None and manifest['commit']!=expected_commit)
            or set(files)!=set(manifest['files'])):
        raise RuntimeError('Review manifest differs')
    for name,value in files.items():
        if manifest['files'][name] != {'sha256':digest(value),'bytes':len(value),'mode':modes[name]}:
            raise RuntimeError('Review artifact damaged')
    source={name[7:]:value for name,value in files.items() if name.startswith('source/')}
    if (manifest['source_files']!=len(source) or files['REVIEW.md']!=source['docs/judge-review.md']
            or manifest['lock_sha256']!={n:digest(source[n]) for n in ('Cargo.lock','tools/nu7-compat/Cargo.lock')}
            or manifest['canonical']!=public_links(source)):
        raise RuntimeError('Review source/public links differ')
    return {'passed':True,'commit':manifest['commit'],'source_files':len(source),
        'canonical':manifest['canonical'],'integrity_only':True,
        'second_machine_reproduction':False,'private_runtime_included':False}


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='action',required=True)
    build=sub.add_parser('create'); build.add_argument('--revision',default='HEAD')
    build.add_argument('--output',required=True,type=pathlib.Path)
    audit=sub.add_parser('check'); audit.add_argument('archive',type=pathlib.Path)
    audit.add_argument('--expected-commit')
    args=parser.parse_args()
    result=create(ROOT,args.revision,args.output) if args.action=='create' else check(args.archive,args.expected_commit)
    print(json.dumps(result,indent=2))


if __name__=='__main__': main()
