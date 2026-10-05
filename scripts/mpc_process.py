"""MPC children inherit a resource guard's group, otherwise own isolated groups."""
import os
import signal

GROUP_ENV = 'DARKCALL_GUARDED_PROCESS_GROUP'


def separate_session():
    # Bind to the actual pgid, not merely a caller-set boolean. Unguarded runners
    # retain independent child groups; guarded nested runners must not escape it.
    return os.environ.get(GROUP_ENV) != str(os.getpgrp())


def signal_child(process, sig=signal.SIGTERM):
    try:
        if os.getpgid(process.pid) == process.pid:
            os.killpg(process.pid, sig)  # This child created its own group.
        else:
            process.send_signal(sig)  # Never signal the shared parent group here.
    except ProcessLookupError:
        pass
