"""Lightweight mechanism reproduction, not a Rust regression or real Codex run."""
import os
from pathlib import Path
import select
import signal
import subprocess
import sys


def exited(fd):
    return bool(select.select([fd], [], [], 0)[0])


def main():
    if len(sys.argv) > 1:
        if sys.argv[1] == 'native':
            sys.stdin.buffer.read()  # Retained RPC stdin keeps the native alive.
        else:
            child = subprocess.Popen([sys.executable, __file__, 'native'])
            print(child.pid, flush=True)
            child.wait()
        return
    for reason in ['timeout', 'stream-drop', 'abort']:
        unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
        wrapper = subprocess.Popen([sys.executable, __file__, 'wrapper'],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   start_new_session=True)
        root_fd = os.pidfd_open(wrapper.pid)
        native_fd = None
        try:
            assert select.select([wrapper.stdout], [], [], 5)[0]
            native = int(wrapper.stdout.readline())
            native_fd = os.pidfd_open(native)
            # Mirrors direct-child kill_on_drop after cancellation bypasses cleanup.
            signal.pidfd_send_signal(root_fd, signal.SIGKILL)
            wrapper.wait(timeout=5)
            assert not exited(native_fd), 'Expected native survivor'
            assert unrelated.poll() is None
            print(f'REPRODUCED {reason}: wrapper exited, native remains, unrelated survives', flush=True)
        finally:
            if native_fd is not None:
                signal.pidfd_send_signal(native_fd, signal.SIGKILL)
                assert select.select([native_fd], [], [], 5)[0]
                os.close(native_fd)
            if wrapper.poll() is None:
                signal.pidfd_send_signal(root_fd, signal.SIGKILL)
            wrapper.wait(timeout=5)
            os.close(root_fd)
            wrapper.stdin.close()
            wrapper.stdout.close()
            unrelated.terminate()
            unrelated.wait(timeout=5)
    print('All fixture processes confirmed exited; no provider/model or live state used.')


if __name__ == '__main__':
    main()
