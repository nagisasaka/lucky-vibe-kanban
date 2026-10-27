"""Test-owned PTY parent; never accesses LVK data or shared instances."""
import ctypes
import json
import os
from pathlib import Path
import signal
import sys
import time

root = Path(sys.argv[1])
mode = sys.argv[2]
# Keep fixture grandchildren test-owned even if a shell exits first.
if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
    raise OSError(ctypes.get_errno(), 'fixture subreaper unavailable')
pid, master = os.forkpty()
if pid == 0:
    env = dict(os.environ, LVK_INSTANCE_DIR=str(root), VIBE_KANBAN_TERMINAL='1')
    if mode == 'foreign':
        env['LVK_INSTANCE_DIR'] = str(root / 'other-instance')
    if mode == 'unmanaged':
        env.pop('VIBE_KANBAN_TERMINAL')
    os.execve('/bin/sh', ['/bin/sh'], env)
if mode in ('command', 'ignore', 'nested'):
    command = {'command': 'sleep 120', 'ignore': "trap '' TERM; sleep 120", 'nested': "sh -c 'sleep 120 & wait'"}[mode]
    os.write(master, (command + '\n').encode())
def cleanup(*_):
    import terminal_drain as drain
    deadline = time.monotonic() + 5
    while True:
        rows = drain.processes()
        children = drain.s.descendants(os.getpid())
        for child in children:
            try:
                fd = os.pidfd_open(child)
                try:
                    if drain.stable(rows[child], drain.processes()[child]):
                        signal.pidfd_send_signal(fd, signal.SIGKILL)
                finally:
                    os.close(fd)
            except (ProcessLookupError, KeyError):
                pass
        try:
            while os.waitpid(-1, os.WNOHANG)[0]:
                pass
        except ChildProcessError:
            break
        if time.monotonic() > deadline:
            raise RuntimeError('fixture children did not settle')
        time.sleep(.01)
    os.close(master)
    sys.exit(0)

signal.signal(signal.SIGTERM, cleanup)
(root / 'ready.tmp').write_text(json.dumps({'parent': os.getpid(), 'shell': pid}))
(root / 'ready.tmp').replace(root / 'ready.json')
while True:
    if mode == 'legacy':
        time.sleep(.01)
        continue
    try:
        reaped, _ = os.waitpid(-1, os.WNOHANG)
        if reaped == pid:
            (root / 'reaped').touch()
    except ChildProcessError:
        pass
    time.sleep(.01)
