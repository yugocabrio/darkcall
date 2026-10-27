"""MPC children inherit a resource guard's group, otherwise own isolated groups."""
import os
import re
import signal
import sys

GROUP_ENV = 'DARKCALL_GUARDED_PROCESS_GROUP'
# Each MPC child runs under time(1) for its peak memory: BSD time on macOS
# reports bytes, GNU time on Linux (the `time` package) kilobytes.
TIME = ['/usr/bin/time', '-l' if sys.platform == 'darwin' else '-v']


def separate_session():
    # Bind to the actual pgid, not merely a caller-set boolean. Unguarded runners
    # retain independent child groups; guarded nested runners must not escape it.
    return os.environ.get(GROUP_ENV) != str(os.getpgrp())


def peak_rss(text):
    """Peak resident bytes from a TIME report, or None."""
    found = [int(n) for n in re.findall(r'(\d+)\s+maximum resident set size', text)]
    found += [int(n) * 1024 for n in re.findall(r'Maximum resident set size \(kbytes\): (\d+)', text)]
    return found[-1] if found else None


def signal_child(process, sig=signal.SIGTERM):
    try:
        if os.getpgid(process.pid) == process.pid:
            os.killpg(process.pid, sig)  # This child created its own group.
        else:
            process.send_signal(sig)  # Never signal the shared parent group here.
    except ProcessLookupError:
        pass
