"""Only test-owned Linux process trees and PTYs receive signals in this suite."""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import process_drain as drain
import start_support as s


class ProcessDrainTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='lvk-process-drain-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.addCleanup(setattr, s, 'PORT', s.PORT)
        self.addCleanup(setattr, s, 'DEV', s.DEV)
        s.configure(self.root, 0)

    def fixture(self, mode='shell'):
        directory = self.root / str(len(list(self.root.iterdir())))
        directory.mkdir()
        child = subprocess.Popen([sys.executable, str(Path(__file__).with_name('fixture_pty.py')),
                                  str(directory), mode], start_new_session=True)
        def cleanup():
            if child.poll() is None:
                child.terminate()
            child.wait(timeout=5)
        self.addCleanup(cleanup)
        deadline = time.monotonic() + 5
        while not (directory / 'ready.json').exists():
            self.assertIsNone(child.poll())
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        shell = json.loads((directory / 'ready.json').read_text())['shell']
        # A different directory normally means a different instance. Bind this
        # fixture explicitly; no shared/live instance configuration is changed.
        s.configure(directory, 0)
        while shell not in s.descendants(child.pid):
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        if mode in ('command', 'ignore', 'nested'):
            while not s.descendants(shell):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.01)
        return child, shell

    def stop(self, process, operation=None):
        s.stop(s.identity(process.pid), s.digest(Path(f'/proc/{process.pid}/exe')),
               require_port=False, operation=operation)
        process.wait(timeout=5)

    def test_empty_and_unmanaged_shells_stop_with_server(self):
        for mode in ('shell', 'unmanaged'):
            with self.subTest(mode=mode):
                parent, shell = self.fixture(mode)
                self.stop(parent)
                self.assertNotEqual(drain.processes().get(shell, {}).get('state'), 'S')

    def test_nested_commands_and_term_ignoring_jobs_exit(self):
        for mode in ('command', 'nested', 'ignore'):
            with self.subTest(mode=mode):
                parent, shell = self.fixture(mode)
                if mode == 'nested':
                    deadline = time.monotonic() + 5
                    while len(s.descendants(shell)) < 2:
                        self.assertLess(time.monotonic(), deadline)
                        time.sleep(.01)
                targets = s.descendants(parent.pid)
                self.stop(parent)
                rows = drain.processes()
                self.assertFalse(any(rows.get(p, {}).get('state', 'Z') != 'Z' for p in targets))

    def test_foreign_instance_is_refused_before_any_signal(self):
        parent, shell = self.fixture('foreign')
        # Wait until the shell has exec'd and published its explicit boundary.
        deadline = time.monotonic() + 5
        while s.process_files(Path(f'/proc/{shell}'))[0].get(b'LVK_INSTANCE_DIR') != str(s.DEV / 'other-instance').encode():
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        with patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaisesRegex(s.Refuse, 'another instance'):
                self.stop(parent)
        send.assert_not_called()
        self.assertIsNone(parent.poll())
        self.assertIn(shell, s.descendants(parent.pid))

    def test_unrelated_parent_and_its_pty_are_preserved(self):
        parent, shell = self.fixture()
        other, other_shell = self.fixture()
        s.configure(self.root / '0', 0)
        self.stop(parent)
        self.assertIsNone(other.poll())
        self.assertIn(other_shell, s.descendants(other.pid))

    def test_server_identity_mismatch_never_signals(self):
        parent, _ = self.fixture()
        want = s.identity(parent.pid)
        want['start'] = 'invalid'
        with patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaises(s.Refuse):
                s.stop(want, s.digest(Path(f'/proc/{parent.pid}/exe')), require_port=False)
        send.assert_not_called()

    def test_changed_child_identity_never_signals(self):
        parent, _ = self.fixture()
        real_stable = drain.stable
        def changed(a, b):
            return False if a['parent'] == parent.pid else real_stable(a, b)
        with patch.object(drain, 'stable', side_effect=changed), patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaisesRegex(s.Refuse, 'identity changed'):
                self.stop(parent)
        send.assert_not_called()

    def test_changed_parent_edge_is_refused_before_signals(self):
        parent, shell = self.fixture()
        original = drain.processes
        calls = 0
        def changed():
            nonlocal calls
            calls += 1
            rows = original()
            if calls >= 2 and shell in rows:
                rows[shell]['parent'] = os.getpid()
            return rows
        with patch.object(drain, 'processes', side_effect=changed), patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaisesRegex(s.Refuse, 'ancestry changed'):
                self.stop(parent)
        send.assert_not_called()
        self.assertIsNone(parent.poll())

    def test_foreign_uid_refuses_before_signals(self):
        parent, shell = self.fixture()
        original = drain.processes
        def changed():
            rows = original()
            rows[shell]['uid'] += 1
            return rows
        with patch.object(drain, 'processes', side_effect=changed), patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaisesRegex(s.Refuse, 'owner mismatch'):
                self.stop(parent)
        send.assert_not_called()

    def test_pinned_reparented_grandchildren_still_receive_kill(self):
        parent, shell = self.fixture('nested')
        deadline = time.monotonic() + 5
        while len(s.descendants(shell)) < 2:
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        old_rows = drain.processes()
        targets = s.descendants(parent.pid)
        real_signal = signal.pidfd_send_signal
        observed_reparent = []
        def signal_and_observe(fd, sig, *args):
            if sig == signal.SIGKILL:
                # Read only the fixture pidfds supplied by the production code.
                info = Path(f'/proc/self/fdinfo/{fd}').read_text()
                target = int(next(line.split()[1] for line in info.splitlines() if line.startswith('Pid:')))
                now = drain.processes().get(target)
                if now and target in old_rows and now['parent'] != old_rows[target]['parent']:
                    observed_reparent.append(target)
                result = real_signal(fd, sig, *args)
                self.assertTrue(select.select([fd], [], [], 2)[0])
                return result
            return real_signal(fd, sig, *args)
        with patch.object(signal, 'pidfd_send_signal', side_effect=signal_and_observe):
            self.stop(parent)
        self.assertTrue(observed_reparent)
        rows = drain.processes()
        self.assertFalse(any(rows.get(p, {}).get('state', 'Z') != 'Z' for p in targets))

    def test_unconfirmed_kill_resumes_parent_and_refuses(self):
        parent, shell = self.fixture()
        real_signal = signal.pidfd_send_signal
        def ignore_kill(fd, sig, *args):
            if sig != signal.SIGKILL:
                return real_signal(fd, sig, *args)
        with patch.object(signal, 'pidfd_send_signal', side_effect=ignore_kill):
            fd = os.pidfd_open(parent.pid)
            try:
                with self.assertRaisesRegex(s.Refuse, 'KILL timeout'):
                    drain.terminate(s.identity(parent.pid), fd, timeout=.2)
            finally:
                os.close(fd)
        self.assertIsNone(parent.poll())
        self.assertNotEqual(drain.processes()[parent.pid]['state'], 'T')
        self.assertNotEqual(drain.processes()[shell]['state'], 'T')


if __name__ == '__main__':
    unittest.main()
