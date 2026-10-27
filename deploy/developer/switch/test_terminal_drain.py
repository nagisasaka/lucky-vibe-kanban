"""Isolated PTY ownership and failure regressions, with real Linux pidfds."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import start_support as s
import terminal_drain as drain


class TerminalDrainTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='lvk-terminal-drain-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.old_instance = s.DEV
        self.addCleanup(setattr, s, 'PORT', s.PORT)
        self.addCleanup(setattr, s, 'DEV', self.old_instance)
        s.configure(self.root, 0)

    def fixture(self, mode='shell'):
        (self.root / 'ready.json').unlink(missing_ok=True)
        process = subprocess.Popen([sys.executable, str(Path(__file__).with_name('fixture_pty.py')),
                                    str(self.root), mode], start_new_session=True)
        def cleanup():
            process.terminate()
            process.wait(timeout=5)
        self.addCleanup(cleanup)
        deadline = time.monotonic() + 5
        while not (self.root / 'ready.json').exists():
            self.assertIsNone(process.poll())
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        shell = json.loads((self.root / 'ready.json').read_text())['shell']
        while shell not in drain.owned_roots(process.pid, drain.processes()):
            if mode in ('foreign', 'unmanaged'):
                break
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        if mode in ('command', 'ignore', 'nested'):
            while not s.descendants(shell):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.01)
        return process, shell

    def test_empty_shell_exits_before_return(self):
        process, shell = self.fixture()
        drain.terminate(s.identity(process.pid), self.root)
        self.assertEqual(drain.processes().get(shell, {}).get('state', 'Z'), 'Z')
        self.assertIsNone(process.poll())

    def test_legacy_parent_without_wait_keeps_only_fd_free_zombie(self):
        process, shell = self.fixture('legacy')
        drain.terminate(s.identity(process.pid), self.root)
        self.assertEqual(drain.processes()[shell]['state'], 'Z')
        self.assertEqual(s.process_files(Path(f'/proc/{shell}')), ({}, []))
        s.idle(process.pid)

    def test_another_servers_managed_pty_is_preserved(self):
        process, shell = self.fixture()
        other, other_shell = self.fixture()
        drain.terminate(s.identity(process.pid), self.root)
        rows = drain.processes()
        self.assertEqual(rows.get(shell, {}).get('state', 'Z'), 'Z')
        self.assertIn(other_shell, rows)
        self.assertNotEqual(rows[other_shell]['state'], 'Z')
        self.assertIsNone(other.poll())

    def test_running_command_exits_before_guards(self):
        process, shell = self.fixture('command')
        children = s.descendants(shell)
        drain.terminate(s.identity(process.pid), self.root)
        rows = drain.processes()
        self.assertEqual(rows.get(shell, {}).get('state', 'Z'), 'Z')
        self.assertFalse(any(rows.get(p, {}).get('state', 'Z') != 'Z' for p in children))
        self.assertFalse(s.descendants(process.pid))

    def test_nested_command_descendants_exit(self):
        process, shell = self.fixture('nested')
        deadline = time.monotonic() + 3
        while len(s.descendants(shell)) < 2:
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.01)
        children = s.descendants(shell)
        drain.terminate(s.identity(process.pid), self.root)
        rows = drain.processes()
        self.assertFalse(any(rows.get(p, {}).get('state', 'Z') != 'Z' for p in {shell} | children))

    def test_term_ignored_refuses_without_kill(self):
        process, shell = self.fixture('ignore')
        with self.assertRaisesRegex(s.Refuse, 'TERM timeout'):
            drain.terminate(s.identity(process.pid), self.root, timeout=.3)
        rows = drain.processes()
        self.assertIn(shell, rows)
        self.assertNotEqual(rows[shell]['state'], 'T')
        self.assertTrue(s.descendants(shell))
        self.assertTrue((self.root / 'terminal-drain.json').exists())

    def test_detached_descendant_is_refused_and_preserved(self):
        rows = {
            100: dict(state='S', parent=1, group=100, session=100, tty=123, start='1', uid=42),
            101: dict(state='S', parent=100, group=101, session=101, tty=0, start='2', uid=42),
        }
        with self.assertRaisesRegex(s.Refuse, 'boundary'):
            drain.members(rows, {100})

    def test_external_member_of_job_group_is_refused(self):
        rows = {
            100: dict(state='S', parent=1, group=100, session=100, tty=123, start='1', uid=42),
            200: dict(state='S', parent=1, group=100, session=200, tty=456, start='2', uid=42),
        }
        with self.assertRaisesRegex(s.Refuse, 'process group crosses'):
            drain.members(rows, {100})

    def test_preexisting_stopped_shell_is_not_resumed(self):
        process, shell = self.fixture()
        fd = os.pidfd_open(shell)
        try:
            signal.pidfd_send_signal(fd, signal.SIGSTOP)
            deadline = time.monotonic() + 2
            while drain.processes()[shell]['state'] != 'T':
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.01)
            with self.assertRaisesRegex(s.Refuse, 'already stopped'):
                drain.terminate(s.identity(process.pid), self.root)
            self.assertEqual(drain.processes()[shell]['state'], 'T')
            signal.pidfd_send_signal(fd, signal.SIGCONT)
        finally:
            os.close(fd)

    def test_foreign_instance_is_preserved(self):
        process, shell = self.fixture('foreign')
        drain.terminate(s.identity(process.pid), self.root)
        self.assertIn(shell, drain.processes())
        with self.assertRaises(s.Refuse):
            s.idle(process.pid)

    def test_unmanaged_pty_is_preserved(self):
        process, shell = self.fixture('unmanaged')
        drain.terminate(s.identity(process.pid), self.root)
        self.assertIn(shell, drain.processes())

    def test_server_identity_mismatch_never_signals(self):
        process, shell = self.fixture()
        want = s.identity(process.pid)
        want['start'] = 'invalid'
        with self.assertRaises(s.Refuse), patch.object(signal, 'pidfd_send_signal') as send:
            drain.terminate(want, self.root)
        send.assert_not_called()
        self.assertIn(shell, drain.processes())

    def test_changed_member_identity_refuses_before_signal(self):
        process, shell = self.fixture()
        with patch.object(drain, 'stable', return_value=False), patch.object(signal, 'pidfd_send_signal') as send:
            with self.assertRaisesRegex(s.Refuse, 'PID identity changed'):
                drain.terminate(s.identity(process.pid), self.root)
        send.assert_not_called()
        self.assertIn(shell, drain.processes())



if __name__ == '__main__':
    unittest.main()
