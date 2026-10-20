"""Isolated actual-process tests. Never invoke the production entry point."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('cutover', Path(__file__).with_name('local9_cutover.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class MockCutover(m.Cutover):
    def frozen(self):
        m.require((self.root / 'frozen').read_text() == 'unchanged', 'Frozen state changed')

    def coordination(self):
        pass

    def idle(self, pid):
        m.require(not (self.root / 'busy').exists(), 'Busy')

    def preflight(self):
        m.require(m.sha(self.live) == self.old and m.sha(self.candidate) == self.new, 'Hash mismatch')
        self.frozen()
        pid = self.pid()
        self.identity(pid, self.old)
        self.owns_listener(pid)
        self.idle(pid)
        self.env = {os.fsencode(k): os.fsencode(v) for k, v in os.environ.items()}
        return pid


class CutoverTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='local9-cutover-test-')
        self.base = Path(self.temp.name)
        self.engine = MockCutover()
        e = self.engine
        e.root = self.base / 'instance'
        e.live = self.base / 'repo/target/debug/server'
        e.candidate = self.base / 'candidate'
        e.directory = self.base / 'result'
        e.stop_timeout = 3
        e.start_timeout = 3
        e.live.parent.mkdir(parents=True)
        (e.root / 'logs').mkdir(parents=True)
        (e.root / 'data').mkdir()
        with socket.socket() as s:
            s.bind(('127.0.0.1', 0)); e.port = s.getsockname()[1]
        shutil.copy2('/usr/bin/python3', e.live)
        shutil.copy2(e.live, e.candidate)
        with e.candidate.open('ab') as f:
            f.write(b'LOCAL9_CANDIDATE')
        e.old, e.new = m.sha(e.live), m.sha(e.candidate)
        (e.root / 'start.lock').touch()
        (e.root / 'frozen').write_text('unchanged')
        (e.root / 'deployed-build.json').write_text('{}')
        (e.root / 'run.py').write_text('# mock launcher\n')
        db = sqlite3.connect(e.root / 'data/db.v2.sqlite')
        db.execute('CREATE TABLE evidence(value TEXT)'); db.execute("INSERT INTO evidence VALUES('preserve')")
        db.commit(); db.close()
        service = self.base / 'service.py'
        service.write_text('''import http.server, json, sys
from pathlib import Path
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  bad = Path(sys.executable).read_bytes().endswith(b'LOCAL9_FAIL')
  self.send_response(503 if bad else 200); self.end_headers()
  self.wfile.write(json.dumps({'data':'BAD' if bad else 'OK'}).encode())
 def log_message(self, *args): pass
print('Main server on :' + sys.argv[1], flush=True)
http.server.HTTPServer(('127.0.0.1', int(sys.argv[1])), Handler).serve_forever()
''')
        start = e.root / 'start.sh'
        start.write_text(f'''#!/bin/bash
set -eu
cd '{e.live.parent.parent.parent}'
nohup '{e.live}' '{service}' '{e.port}' >> '{e.root}/logs/server.log' 2>&1 < /dev/null &
echo $! > '{e.root}/server.pid'
''')
        start.chmod(0o700)
        e.directory.mkdir()
        e.env = dict(os.environ)
        self.oldpid = e.start(e.old)
        shutil.rmtree(e.directory)

    def tearDown(self):
        # Only stop our verified fixture executable; never broad process cleanup.
        for name in ('server.pid',):
            path = self.engine.root / name
            if path.exists():
                pid = int(path.read_text())
                if self.engine.alive(pid):
                    expected = m.sha(f'/proc/{pid}/exe')
                    if os.readlink(f'/proc/{pid}/exe') == str(self.engine.live):
                        self.engine.stop(pid, expected)
        if self.engine.alive(self.oldpid):
            self.engine.stop(self.oldpid, self.engine.old)
        self.temp.cleanup()

    def test_success_and_repeat_refused(self):
        self.assertEqual(self.engine.run(), 0)
        self.assertEqual(m.sha(self.engine.live), self.engine.new)
        self.assertEqual(m.sha(self.engine.directory / 'server.old'), self.engine.old)
        db = sqlite3.connect(self.engine.directory / 'db.v2.consistent.sqlite')
        self.assertEqual(db.execute('SELECT value FROM evidence').fetchone()[0], 'preserve'); db.close()
        with self.assertRaises(FileExistsError):
            self.engine.run()

    def test_failed_health_rolls_back(self):
        with self.engine.candidate.open('ab') as f:
            f.write(b'LOCAL9_FAIL')
        self.engine.new = m.sha(self.engine.candidate)
        self.assertEqual(self.engine.run(), 3)
        self.assertEqual(m.sha(self.engine.live), self.engine.old)
        self.assertEqual(json.loads((self.engine.directory / 'state.json').read_text())['stage'], 'ROLLED_BACK_NO_DB_RESTORE')

    def test_stale_pid_aborts_without_signal(self):
        (self.engine.root / 'server.pid').write_text(str(os.getpid()))
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))
        (self.engine.root / 'server.pid').write_text(str(self.oldpid))

    def test_changed_payload_aborts(self):
        (self.engine.root / 'frozen').write_text('changed')
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_changed_artifact_aborts(self):
        with self.engine.candidate.open('ab') as output:
            output.write(b'tampered')
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_busy_aborts(self):
        (self.engine.root / 'busy').touch()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))


if __name__ == '__main__':
    unittest.main()
