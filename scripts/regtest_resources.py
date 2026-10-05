"""Bound a local proof subprocess and preserve resource observations privately.

This stops only the newly created process group. It does not reset journals,
remove artifacts or establish that hardware/OS crashes cannot occur.
"""
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

MIN_FREE_BYTES = 4 * 1024 ** 3
# A measured native row transiently reached 6.14 GiB while macOS reported 70%
# headroom. Keep one prover, a half-of-16-GiB resident ceiling, and independent
# system/disk guards rather than interrupt that observed legitimate peak.
MAX_RESIDENT_KIB = 8 * 1024 ** 2
MIN_MAC_FREE_PERCENT = 10


def sample(directory, pid=None):
    result = {'free_disk_bytes': shutil.disk_usage(directory).free}
    if pid is not None:
        observed = subprocess.run(['ps', '-o', 'rss=', '-p', str(pid)],
                                  capture_output=True, text=True, timeout=5)
        if observed.returncode == 0 and observed.stdout.strip():
            result['prover_resident_kib'] = int(observed.stdout.strip())
    if sys.platform == 'darwin':
        observed = subprocess.run(['/usr/bin/memory_pressure', '-Q'],
                                  capture_output=True, text=True, timeout=5)
        if observed.returncode != 0:
            raise RuntimeError('Could not observe system memory headroom')
        match = re.search(r'System-wide memory free percentage: (\d+)%', observed.stdout)
        if match is None:
            raise RuntimeError('Unrecognized memory headroom observation')
        result['system_memory_free_percent'] = int(match[1])
    return result


def check(observation):
    if observation['free_disk_bytes'] < MIN_FREE_BYTES:
        raise RuntimeError('Proof stopped: less than 4 GiB disk headroom; artifacts retained')
    if observation.get('prover_resident_kib', 0) > MAX_RESIDENT_KIB:
        raise RuntimeError('Proof stopped: prover exceeds 8 GiB resident memory; artifacts retained')
    if observation.get('system_memory_free_percent', 100) < MIN_MAC_FREE_PERCENT:
        raise RuntimeError('Proof stopped: system memory headroom below 10%; artifacts retained')


def stop(process):
    # Descendants may still be alive after the parent exits. Kill only this
    # invocation's fresh session/group, including those descendants.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def guarded_json_call(command, *, cwd, env, timeout, observations, interval=10):
    observations = pathlib.Path(observations)
    initial = sample(cwd)
    check(initial)  # Do not launch a proof when the host is already constrained.
    fd = os.open(observations, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    started = time.monotonic()
    with os.fdopen(fd, 'w') as metrics, tempfile.TemporaryFile() as output:
        def record(value):
            metrics.write(json.dumps({'elapsed_seconds': round(time.monotonic() - started, 3), **value}) + '\n')
            metrics.flush()
            os.fsync(metrics.fileno())
        record(initial)
        process = subprocess.Popen(command, cwd=cwd, env=env, stdout=output, start_new_session=True)
        try:
            while True:
                try:
                    code = process.wait(timeout=min(interval, max(0.01, timeout - (time.monotonic() - started))))
                    break
                except subprocess.TimeoutExpired:
                    value = sample(cwd, process.pid)
                    record(value)
                    check(value)
                    if time.monotonic() - started >= timeout:
                        raise RuntimeError('Proof timeout; private artifacts and journals retained')
            if code != 0:
                raise RuntimeError('Local proof process failed; inspect private log/artifacts')
            output.seek(0)
            data = output.read(4 * 1024 ** 2 + 1)
            if len(data) > 4 * 1024 ** 2:
                raise RuntimeError('Local proof summary exceeds bound')
            result = json.loads(data)
            record({'completed': True})
            return result
        except BaseException:
            stop(process)
            raise
