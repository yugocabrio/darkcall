"""Exercise host-bound proof interruption without touching chain/funding files."""
import json
import os
import pathlib
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import regtest_resources as resources
from mpc_process import signal_child


class ResourceTests(unittest.TestCase):
    def test_stage_cleanup_signals_only_its_child_or_owned_group(self):
        # Both standalone and guarded-style groups must leave this parent alive.
        for isolated in (True, False):
            child = subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)'],
                start_new_session=isolated,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            try:
                signal_child(child)
                self.assertNotEqual(child.wait(timeout=3), 0)
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=3)

    def test_guard_abort_also_stops_nested_mpc_children(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            code = '''import pathlib,subprocess,sys,time
from mpc_process import separate_session
child_code="import os,pathlib,time;pathlib.Path('grandchild-pid').write_text(str(os.getpid()));time.sleep(30)"
child=subprocess.Popen(['/usr/bin/time','-l' if sys.platform=='darwin' else '-v',sys.executable,'-c',child_code],
    start_new_session=separate_session(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
pathlib.Path('child-pid').write_text(str(child.pid))
time.sleep(30)
'''
            env = {**os.environ, 'PYTHONPATH': str(pathlib.Path(__file__).resolve().parent)}
            plenty = {'free_disk_bytes': resources.MIN_FREE_BYTES + 1}
            pid = None
            try:
                with patch.object(resources, 'sample', side_effect=[plenty, {'free_disk_bytes': 0}]):
                    with self.assertRaisesRegex(RuntimeError, 'disk headroom'):
                        resources.guarded_json_call([sys.executable, '-c', code],
                            cwd=path, env=env, timeout=5, observations=path/'metrics', interval=.4)
                pid = int((path/'child-pid').read_text())
                for observed_pid in (pid, int((path/'grandchild-pid').read_text())):
                    state = subprocess.run(['ps','-o','stat=','-p',str(observed_pid)],
                        capture_output=True,text=True,timeout=5).stdout.strip()
                    self.assertTrue(not state or state.startswith('Z'),
                                    'Nested MPC-like child survives parent guard interruption')
            finally:
                # Clean only this test's explicitly recorded child even on failure.
                if pid is None and (path/'child-pid').exists():
                    pid = int((path/'child-pid').read_text())
                for filename in ('child-pid','grandchild-pid'):
                    if (path/filename).exists():
                        try: os.kill(int((path/filename).read_text()),signal.SIGKILL)
                        except ProcessLookupError: pass

    def test_expected_failure_requires_exact_diagnostic_and_private_log(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            plenty = {'free_disk_bytes': resources.MIN_FREE_BYTES + 1}
            for i, (code, message, passes) in enumerate(((1, 'bad cached proof', True),
                                                       (1, 'unrelated failure', False),
                                                       (0, 'bad cached proof', False))):
                with patch.object(resources, 'sample', return_value=plenty):
                    args = dict(cwd=path, env=os.environ, timeout=5,
                                observations=path / f'metrics-{i}',
                                expected_failure='bad cached proof', failure_log=path / f'log-{i}')
                    command = [sys.executable, '-c',
                               f'import sys; print({message!r},file=sys.stderr); sys.exit({code})']
                    if passes:
                        self.assertEqual(resources.guarded_json_call(command, **args),
                                         {'expected_rejection': True, 'returncode': 1})
                    else:
                        with self.assertRaisesRegex(RuntimeError, 'not observed'):
                            resources.guarded_json_call(command, **args)
                self.assertEqual((path / f'log-{i}').stat().st_mode & 0o777, 0o600)

    def test_swap_observation_is_used_bytes_not_total_allocation(self):
        self.assertEqual(resources.swap_used_bytes(
            'vm.swapusage: total = 7168.00M used = 6117.31M free = 1050.69M (encrypted)'),
            int(6117.31 * 1024 ** 2))
        self.assertEqual(resources.swap_used_bytes('used = 1.5G'), 1610612736)
        with self.assertRaisesRegex(RuntimeError, 'swap observation'):
            resources.swap_used_bytes('used = unavailable')

    def test_limits_and_supported_missing_platform_metrics(self):
        resources.check({'free_disk_bytes': resources.MIN_FREE_BYTES})
        for value in ({'free_disk_bytes': 0},
                      {'free_disk_bytes': resources.MIN_FREE_BYTES,
                       'prover_resident_kib': resources.MAX_RESIDENT_KIB + 1},
                      {'free_disk_bytes': resources.MIN_FREE_BYTES,
                       'system_memory_free_percent': 9}):
            with self.assertRaises(RuntimeError):
                resources.check(value)

    def test_preflight_does_not_launch_or_modify_existing_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            marker = path / 'child-ran'
            with patch.object(resources, 'sample', return_value={'free_disk_bytes': 0}):
                with self.assertRaises(RuntimeError):
                    resources.guarded_json_call([sys.executable, '-c',
                        'from pathlib import Path; Path("child-ran").touch()'],
                        cwd=path, env=os.environ, timeout=5, observations=path / 'metrics')
            self.assertFalse(marker.exists())
            self.assertFalse((path / 'metrics').exists())

    def test_child_is_stopped_on_resource_loss_and_completed_json_is_returned(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            plenty = {'free_disk_bytes': resources.MIN_FREE_BYTES + 1}
            with patch.object(resources, 'sample', side_effect=[plenty, {'free_disk_bytes': 0}]):
                with self.assertRaisesRegex(RuntimeError, 'disk headroom'):
                    resources.guarded_json_call([sys.executable, '-c', 'import time; time.sleep(30)'],
                        cwd=path, env=os.environ, timeout=5, observations=path / 'failed', interval=0.1)
            self.assertEqual(len((path / 'failed').read_text().splitlines()), 2)
            self.assertEqual((path / 'failed').stat().st_mode & 0o777, 0o600)
            with patch.object(resources, 'sample', return_value=plenty):
                result = resources.guarded_json_call([sys.executable, '-c', 'print("{\\"ok\\":true}")'],
                    cwd=path, env=os.environ, timeout=5, observations=path / 'success')
            self.assertEqual(result, {'ok': True})
            self.assertTrue(json.loads((path / 'success').read_text().splitlines()[-1])['completed'])

    def test_stronger_gate_preflight_never_starts_child_or_creates_metrics(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            with patch.object(resources, 'sample', return_value={'free_disk_bytes': resources.MIN_FREE_BYTES}), \
                 patch.object(resources.subprocess, 'Popen') as child:
                with self.assertRaisesRegex(RuntimeError, 'initial disk headroom'):
                    resources.guarded_json_call(['unused'], cwd=path, env=os.environ,
                        timeout=1, observations=path / 'metrics',
                        minimum_start_disk_bytes=16 * 1024 ** 3)
            child.assert_not_called()
            self.assertFalse((path / 'metrics').exists())


if __name__ == '__main__':
    unittest.main()
