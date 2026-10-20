"""Isolated actual-process tests. Never invoke the production entry point."""
import hashlib
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
from unittest.mock import patch
import uuid

spec = importlib.util.spec_from_file_location('cutover', Path(__file__).with_name('local9_cutover.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class MockCutover(m.Cutover):
    def coordination(self):
        pass

    def idle(self, pid):
        self.database_idle()
        m.require(not (self.root / 'busy').exists(), 'Busy')

    def preflight(self):
        m.require(m.sha(self.live) == self.old and m.sha(self.candidate) == self.new, 'Hash mismatch')
        self.source()
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
        source = self.base / 'source'
        source.mkdir()
        def git(*args):
            return subprocess.check_output(['git', '-C', str(source), *args], text=True, stderr=subprocess.DEVNULL).strip()
        git('init', '-b', 'main')
        git('config', 'user.name', 'Fixture')
        git('config', 'user.email', 'fixture@example.invalid')
        (source / 'runtime.rs').write_text('// frozen runtime\n')
        git('add', '.')
        git('commit', '-m', 'fixture')
        base = git('rev-parse', 'HEAD')
        refs = [(name, base) for name, _ in m.REFS]
        for name, _ in refs:
            git('branch', name)
        payload = json.dumps({'skip_validation': True, 'published': False,
                              'publication_intent': False, 'result_commit': None,
                              'base_commit': base, 'sources': [dict(branch=b, commit=c) for b, c in refs]})
        failed_steer = json.dumps({'command': {'type': 'steer', 'content': 'historical fixture'}})
        self.patches = [patch.object(m, k, v) for k, v in {
            'SOURCE': source, 'SOURCE_SHA': base, 'MAIN': base, 'REFS': refs,
            'PAYLOAD': hashlib.sha256(payload.encode()).hexdigest(),
            'FAILED_STEER_HASH': hashlib.sha256(failed_steer.encode()).hexdigest(),
        }.items()]
        for patcher in self.patches:
            patcher.start()
        self.addCleanup(lambda: [p.stop() for p in reversed(self.patches)])
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
        db.executescript("""
            CREATE TABLE agent_runs(id BLOB, status TEXT, projection_status TEXT);
            CREATE TABLE agent_process_registry(registry_status TEXT);
            CREATE TABLE execution_processes(status TEXT);
            CREATE TABLE workflow_runs(status TEXT);
            CREATE TABLE resource_operations(status TEXT);
            CREATE TABLE orchestration_outbox(delivery_status TEXT);
            CREATE TABLE scheduled_tasks(enabled INTEGER);
            CREATE TABLE integration_runs(id BLOB, status TEXT, payload TEXT);
            CREATE TABLE orchestration_inbox(consumption_status TEXT);
            CREATE TABLE orchestration_runs(id BLOB, status TEXT, product_kind TEXT, plan_snapshot TEXT);
            CREATE TABLE orchestration_node_executions(id BLOB, orchestration_run_id BLOB, node_key TEXT, iteration INTEGER, status TEXT);
            CREATE TABLE orchestration_agent_run_links(agent_run_id BLOB, orchestration_run_id BLOB, node_execution_id BLOB);
            CREATE TABLE integration_reservations(resource_kind TEXT,resource_key TEXT,run_id BLOB);
            CREATE TABLE agent_run_commands(command_id BLOB,agent_run_id BLOB,delivery_status TEXT,command_envelope TEXT);
        """)
        run, agent, node = [uuid.UUID(v).bytes for v in (m.RUN, m.AGENT, m.NODE)]
        db.execute("INSERT INTO agent_runs VALUES (?,'succeeded','projection_degraded')", (agent,))
        db.execute("INSERT INTO agent_process_registry VALUES ('exited')")
        db.execute("INSERT INTO orchestration_outbox VALUES ('delivered')")
        db.execute("INSERT INTO integration_runs VALUES (?,'blocked',?)", (run, payload))
        db.execute("INSERT INTO orchestration_runs VALUES (?,'running','integration',?)", (run, json.dumps({'nodes': [{'node_key': 'integrate', 'requires_product_validation': True}]})))
        db.execute("INSERT INTO orchestration_node_executions VALUES (?,?,'integrate',0,'running')", (node, run))
        db.execute('INSERT INTO orchestration_agent_run_links VALUES (?,?,?)', (agent, run, node))
        db.executemany('INSERT INTO integration_reservations VALUES (?,?,?)', m.RESERVATIONS)
        db.execute("INSERT INTO agent_run_commands VALUES (?,?,'failed',?)", (uuid.UUID(m.FAILED_STEER).bytes, agent, failed_steer))
        db.commit(); db.close()
        self.wal_connection = sqlite3.connect(e.root / 'data/db.v2.sqlite')
        self.wal_connection.execute('PRAGMA journal_mode=WAL')
        self.wal_connection.execute("INSERT INTO evidence VALUES ('committed WAL')")
        self.wal_connection.commit()
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
        self.wal_connection.close()
        self.temp.cleanup()

    def test_success_and_repeat_refused(self):
        self.assertEqual(self.engine.run(), 0)
        self.assertEqual(m.sha(self.engine.live), self.engine.new)
        self.assertEqual(m.sha(self.engine.directory / 'server.old'), self.engine.old)
        db = sqlite3.connect(self.engine.directory / 'db.v2.consistent.sqlite')
        self.assertEqual(db.execute('SELECT value FROM evidence').fetchall(), [('preserve',), ('committed WAL',)]); db.close()
        with self.assertRaises(FileExistsError):
            self.engine.run()

    def test_failed_health_rolls_back(self):
        with self.engine.candidate.open('ab') as f:
            f.write(b'LOCAL9_FAIL')
        self.engine.new = m.sha(self.engine.candidate)
        self.assertEqual(self.engine.run(), 3)
        self.assertEqual(m.sha(self.engine.live), self.engine.old)
        self.assertEqual(json.loads((self.engine.directory / 'state.json').read_text())['stage'], 'ROLLED_BACK_NO_DB_RESTORE')

    def test_launcher_failure_rolls_back(self):
        start = self.engine.root / 'start.sh'
        guard = f"python3 -c \"import pathlib,sys; sys.exit(pathlib.Path('{self.engine.live}').read_bytes().endswith(b'LOCAL9_CANDIDATE'))\"\n"
        start.write_text(start.read_text().replace('set -eu\n', 'set -eu\n' + guard))
        self.assertEqual(self.engine.run(), 3)
        self.assertEqual(m.sha(self.engine.live), self.engine.old)

    def test_stale_pid_aborts_without_signal(self):
        (self.engine.root / 'server.pid').write_text(str(os.getpid()))
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))
        (self.engine.root / 'server.pid').write_text(str(self.oldpid))

    def test_changed_payload_aborts(self):
        self.wal_connection.execute("UPDATE integration_runs SET payload='{}'")
        self.wal_connection.commit()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_changed_artifact_aborts(self):
        with self.engine.candidate.open('ab') as output:
            output.write(b'tampered')
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_busy_aborts(self):
        self.wal_connection.execute("INSERT INTO agent_runs VALUES (?,'running','current')", (uuid.uuid4().bytes,))
        self.wal_connection.commit()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_changed_source_aborts(self):
        (m.SOURCE / 'runtime.rs').write_text('// changed source\n')
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_changed_main_aborts(self):
        subprocess.run(['git', '-C', str(m.SOURCE), 'commit', '--allow-empty', '-m', 'changed main'], check=True, stdout=subprocess.DEVNULL)
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_foreign_reservation_aborts(self):
        self.wal_connection.execute('UPDATE integration_reservations SET run_id=?', (uuid.uuid4().bytes,))
        self.wal_connection.commit()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_pending_command_is_not_a_historical_exception(self):
        self.wal_connection.execute("UPDATE agent_run_commands SET delivery_status='pending'")
        self.wal_connection.commit()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_inert_parent_requires_product_gate(self):
        self.wal_connection.execute('UPDATE orchestration_runs SET plan_snapshot=?', (json.dumps({'nodes': [{'node_key': 'integrate', 'requires_product_validation': False}]}),))
        self.wal_connection.commit()
        self.assertEqual(self.engine.run(), 2)
        self.assertTrue(self.engine.alive(self.oldpid))

    def test_exception_text_is_not_logged(self):
        with patch.object(self.engine, 'preflight', side_effect=RuntimeError('fixture-private-value')):
            self.assertEqual(self.engine.run(), 2)
        self.assertNotIn('fixture-private-value', (self.engine.directory / 'cutover.log').read_text())

    def test_preflight_is_read_only(self):
        self.engine.preflight()
        self.assertFalse(self.engine.directory.exists())
        self.assertTrue(self.engine.alive(self.oldpid))


if __name__ == '__main__':
    unittest.main()
