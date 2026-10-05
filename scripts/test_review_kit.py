"""Exercise unpublished archive boundaries, without keys, proof generation or RPC."""
import io
import pathlib
import shutil
import subprocess
import tarfile
import tempfile
import unittest
import review_kit as kit


class ReviewKitTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=pathlib.Path(self.temp.name)
        subprocess.run(['git','init','-q',str(self.root)],check=True)
        subprocess.run(['git','-C',str(self.root),'config','user.email','fixture@example.invalid'],check=True)
        subprocess.run(['git','-C',str(self.root),'config','user.name','Local fixture'],check=True)
        for name in ('Cargo.lock','tools/nu7-compat/Cargo.lock','docs/judge-review.md',kit.AUTHORITY):
            dest=self.root/name; dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(kit.ROOT/name,dest)
        shutil.copytree(kit.ROOT/kit.BUNDLE,self.root/kit.BUNDLE)
        (self.root/'.gitignore').write_text('.local/\n')
        (self.root/'README.md').write_text('Committed source\n')
        subprocess.run(['git','-C',str(self.root),'add','.'],check=True)
        subprocess.run(['git','-C',str(self.root),'commit','-qm','Fixture'],check=True)
        self.commit=kit.git_bytes(self.root,'rev-parse','HEAD').decode().strip()
        (self.root/'.local').mkdir()
        self.out=self.root/'.local/kit.tar.gz'

    def tearDown(self): self.temp.cleanup()

    def repack(self, mutation):
        with tarfile.open(self.out,'r:gz') as original:
            records=[(m,original.extractfile(m).read()) for m in original]
        changed=mutation(records)
        target=self.root/'.local/altered.tar.gz'
        with tarfile.open(target,'w:gz') as archive:
            for member,data in changed: archive.addfile(member,io.BytesIO(data))
        return target

    def test_committed_only_deterministic_and_no_overwrite(self):
        # Fake secret sentinel stays ignored; a modified tracked working file
        # also cannot slip into an archive of an already committed revision.
        (self.root/'.local/private-sentinel.key').write_text('SECRET_SENTINEL')
        (self.root/'README.md').write_text('UNCOMMITTED_SENTINEL')
        result=kit.create(self.root,'HEAD',self.out)
        second=self.root/'.local/second.tar.gz'
        kit.create(self.root,'HEAD',second)
        self.assertEqual(self.out.read_bytes(),second.read_bytes())
        with tarfile.open(self.out,'r:gz') as archive:
            values=[archive.extractfile(m).read() for m in archive]
            names=[m.name for m in archive]
        self.assertFalse(any(b'SECRET_SENTINEL' in v or b'UNCOMMITTED_SENTINEL' in v for v in values))
        self.assertFalse(any('/.git/' in n or '/.local/' in n for n in names))
        self.assertFalse(result['canonical']['proofs_cryptographically_verified'])
        self.assertEqual(self.out.stat().st_mode & 0o777,0o600)
        with self.assertRaises(FileExistsError): kit.create(self.root,'HEAD',self.out)
        with self.assertRaises(RuntimeError): kit.check(self.out,'0'*40)

    def test_archive_traversal_links_duplicates_and_damage_refused(self):
        kit.create(self.root,'HEAD',self.out)
        def extra(records,name,kind=tarfile.REGTYPE):
            member=tarfile.TarInfo(name); member.type=kind; member.mode=0o644
            member.linkname='../elsewhere' if kind==tarfile.SYMTYPE else ''
            return records+[(member,b'')]
        prefix='darkcall-review-'+self.commit[:12]
        for name,kind in [(prefix+'/source/../escape',tarfile.REGTYPE),
                          (prefix+'/source/README.md',tarfile.SYMTYPE),
                          (prefix+'/REVIEW.md',tarfile.REGTYPE),
                          (prefix+'/source/.local/pads.json',tarfile.REGTYPE)]:
            with self.subTest(name=name):
                target=self.repack(lambda records:extra(records,name,kind))
                with self.assertRaises(RuntimeError): kit.check(target)
        def damage(records):
            member,data=records[0]; data=bytes([data[0]^1])+data[1:]
            return [(member,data),*records[1:]]
        with self.assertRaises(RuntimeError): kit.check(self.repack(damage))

    def test_tracked_runtime_and_credential_markers_refused(self):
        for name in ('.local/pads.json','scripts/private-keys.json','circuits/auction4/Prover.toml',
                     'crates/input-tool/target/fixture','evidence/private/run.json',
                     'evidence/wallet.db-wal','evidence/wallet.db-shm'):
            with self.subTest(name=name),self.assertRaises(RuntimeError): kit.safe_source(name,b'{}')
        for value in (b' '.join([b'-----BEGIN',b'RSA',b'PRIVATE',b'KEY-----']),
                      b'https'+b'://user:fixture-password@example.invalid/repo',
                      b'github'+b'_pat_'+b'x'*40):
            with self.assertRaises(RuntimeError): kit.safe_source('README.md',value)
        (self.root/'.local/pads.json').write_text('{}')
        subprocess.run(['git','-C',str(self.root),'add','-f','.local/pads.json'],check=True)
        subprocess.run(['git','-C',str(self.root),'commit','-qm','Deliberately unsafe fixture'],check=True)
        with self.assertRaises(RuntimeError): kit.create(self.root,'HEAD',self.out)
        self.assertFalse(self.out.exists())


if __name__=='__main__': unittest.main()
