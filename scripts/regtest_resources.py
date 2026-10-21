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
from mpc_process import GROUP_ENV

MIN_FREE_BYTES = 4 * 1024 ** 3
# A measured native row transiently reached 6.14 GiB while macOS reported 70%
# headroom. Keep one prover, a half-of-16-GiB resident ceiling, and independent
# system/disk guards rather than interrupt that observed legitimate peak.
MAX_RESIDENT_KIB = 8 * 1024 ** 2
MIN_MAC_FREE_PERCENT = 10


def swap_used_bytes(text):
    """Parse macOS's used swap, separately from resident process memory."""
    match = re.search(r'\bused\s*=\s*([\d.]+)([KMG])', text)
    if match is None:
        raise RuntimeError('Unrecognized macOS swap observation')
    return int(float(match[1]) * {'K': 1024, 'M': 1024 ** 2, 'G': 1024 ** 3}[match[2]])


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
        swap = subprocess.run(['/usr/sbin/sysctl', 'vm.swapusage'],
                              capture_output=True, text=True, timeout=5)
        if swap.returncode != 0:
            raise RuntimeError('Could not observe macOS swap usage')
        result['system_swap_used_bytes'] = swap_used_bytes(swap.stdout)
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
    # macOS answers EPERM, not ESRCH, when only zombies are left in the group;
    # nothing is left to signal then, and the wait below reaps the parent.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except PermissionError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    process.wait()


class TreeSurvived(RuntimeError):
    """Processes from a stopped driver still run; the caller must keep its lock."""


def descendants(root):
    """ROOT and every process it started, from one ps snapshot, across groups
    and sessions (provers run in their own sessions under guarded_json_call)."""
    out = subprocess.run(['ps', '-A', '-o', 'pid=,ppid='], capture_output=True, text=True, check=True).stdout.split()
    children = {}
    for pid, ppid in zip(map(int, out[0::2]), map(int, out[1::2])):
        children.setdefault(ppid, []).append(pid)
    found, queue = set(), [root]
    while queue:
        pid = queue.pop()
        if pid not in found:
            found.add(pid)
            queue.extend(children.get(pid, []))
    return found


def running(pids):
    """Members of PIDS that still run; zombies awaiting reaping do not count."""
    if not pids:
        return set()
    out = subprocess.run(['ps', '-o', 'pid=,stat=', '-p', ','.join(map(str, sorted(pids)))],
                         capture_output=True, text=True).stdout
    alive = set()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 2 and not parts[1].startswith('Z'):
            alive.add(int(parts[0]))
    return alive


def stop_tree(process, grace=30):
    """Stop PROCESS and everything it started, and return only once none of it
    runs. The tree is frozen first, so nothing spawns or is re-parented while it
    is collected; then every member is killed. Raises TreeSurvived otherwise."""
    members = set()
    for _ in range(5):
        found = descendants(process.pid) - members
        if not found:
            break
        members |= found
        for pid in found:
            try:
                os.kill(pid, signal.SIGSTOP)
            except ProcessLookupError:
                pass
    for pid in members:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait()
    deadline = time.monotonic() + grace
    while running(members):
        if time.monotonic() > deadline:
            raise TreeSurvived('Driver processes survived SIGKILL; the proof lock stays held')
        time.sleep(0.2)


def run_bounded(command, *, timeout, input=None, **popen):
    """Like subprocess.run, but a timeout or interruption stops the whole tree."""
    process = subprocess.Popen(command, stdin=subprocess.PIPE if input is not None else None, **popen)
    try:
        out, _ = process.communicate(input=input, timeout=timeout)
    except BaseException:
        stop_tree(process)
        raise
    return subprocess.CompletedProcess(command, process.returncode, out, None)


def guarded_json_call(command, *, cwd, env, timeout, observations, interval=10,
                      minimum_start_disk_bytes=MIN_FREE_BYTES,
                      expected_failure=None, failure_log=None):
    if (expected_failure is None) != (failure_log is None) or expected_failure == '':
        raise ValueError('Expected failure requires both a nonempty diagnostic and private log')
    observations = pathlib.Path(observations)
    initial = sample(cwd)
    check(initial)  # Do not launch a proof when the host is already constrained.
    if initial['free_disk_bytes'] < minimum_start_disk_bytes:
        gib = 1024 ** 3
        message = ('Proof not launched: insufficient initial disk headroom for this measured gate '
                   f'(need {minimum_start_disk_bytes / gib:.2f} GiB, '
                   f'available {initial["free_disk_bytes"] / gib:.2f} GiB')
        if 'system_swap_used_bytes' in initial:
            message += f', used macOS swap {initial["system_swap_used_bytes"] / gib:.2f} GiB'
        raise RuntimeError(message + '); retained artifacts/journals are unchanged')
    fd = os.open(observations, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    started = time.monotonic()
    with os.fdopen(fd, 'w') as metrics, tempfile.TemporaryFile() as output:
        def record(value):
            metrics.write(json.dumps({'elapsed_seconds': round(time.monotonic() - started, 3), **value}) + '\n')
            metrics.flush()
            os.fsync(metrics.fileno())
        record(initial)
        # The fresh session's pgid is the launched child's PID. Set the marker
        # from inside that session so nested MPC runners can verify it before
        # deciding whether to create their own groups. No preexec_fn/fork hooks.
        launch = [sys.executable, '-c',
                  'import os,sys; os.environ[sys.argv[1]]=str(os.getpgrp()); '
                  'os.execvpe(sys.argv[2],sys.argv[2:],os.environ)', GROUP_ENV, *command]
        process = subprocess.Popen(launch, cwd=cwd, env=env, stdout=output,
                                   stderr=subprocess.STDOUT if expected_failure is not None else None,
                                   start_new_session=True)
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
            if code != 0 and expected_failure is None:
                raise RuntimeError('Local proof process failed; inspect private log/artifacts')
            output.seek(0)
            data = output.read(4 * 1024 ** 2 + 1)
            if len(data) > 4 * 1024 ** 2:
                raise RuntimeError('Local proof summary exceeds bound')
            if expected_failure is not None:
                # Negative cryptographic gates retain diagnostics privately and
                # accept only the intended rejection, never an arbitrary crash.
                fd = os.open(failure_log, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'wb') as log:
                    log.write(data)
                    log.flush()
                    os.fsync(log.fileno())
                if code == 0 or expected_failure.encode() not in data:
                    raise RuntimeError('Expected cryptographic rejection was not observed')
                record({'completed': True, 'expected_rejection': True})
                return {'expected_rejection': True, 'returncode': code}
            result = json.loads(data)
            record({'completed': True})
            return result
        except BaseException:
            stop(process)
            raise
