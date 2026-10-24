"""Check Linux WNOWAIT/pidfd primitives; does NOT substitute Rust regression."""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time


def identity(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return tuple(int(fields[i]) for i in (1, 2, 3, 19))


def wait_until(predicate):
    deadline = time.monotonic() + 5
    while not predicate():
        assert time.monotonic() < deadline, 'fixture deadline exceeded'
        time.sleep(.01)


def main():
    fixture = Path(__file__).with_name('fake_app_server.py').resolve()
    for mode in ['normal', 'wrapper-exit']:
        with tempfile.TemporaryDirectory(prefix='owned-group-', dir=Path(__file__).parent) as directory:
            record = Path(directory) / 'pids.json'
            unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
            wrapper = subprocess.Popen([sys.executable, str(fixture), mode, str(record)],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=subprocess.DEVNULL, start_new_session=True)
            root_fd = os.pidfd_open(wrapper.pid)
            native_fd = None
            anchor = identity(wrapper.pid)
            try:
                wait_until(record.exists)
                native = json.loads(record.read_text())['native']
                native_fd = os.pidfd_open(native)
                if mode == 'normal':
                    wrapper.stdin.write(b'{"id":1,"method":"initialize"}\n')
                    wrapper.stdin.flush()
                    assert select.select([wrapper.stdout], [], [], 5)[0]
                    assert json.loads(wrapper.stdout.readline())['id'] == 1
                    signal.pidfd_send_signal(root_fd, signal.SIGKILL)
                wait_until(lambda: bool(os.waitid(os.P_PIDFD, root_fd, os.WEXITED | os.WNOWAIT | os.WNOHANG)))
                assert identity(wrapper.pid) == anchor
                assert not select.select([native_fd], [], [], 0)[0]
                assert identity(native)[1:3] == anchor[1:3]
                # Pinned identities only, never a numeric PGID after reaping.
                signal.pidfd_send_signal(native_fd, signal.SIGKILL)
                wait_until(lambda: bool(select.select([native_fd], [], [], 0)[0]))
                assert identity(wrapper.pid) == anchor
                wrapper.wait(timeout=5)
                assert unrelated.poll() is None
                print(f'PASS {mode}: unreaped anchor retained, owned native exited, unrelated alive')
            finally:
                for fd in (native_fd, root_fd):
                    if fd is not None:
                        try:
                            signal.pidfd_send_signal(fd, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        assert select.select([fd], [], [], 5)[0]
                        os.close(fd)
                wrapper.wait(timeout=5)
                wrapper.stdin.close()
                wrapper.stdout.close()
                unrelated.terminate()
                unrelated.wait(timeout=5)
    print('All fixture processes confirmed exited; no Rust/provider/live execution.')


if __name__ == '__main__':
    main()
