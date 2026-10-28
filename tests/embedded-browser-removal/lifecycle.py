"""Disposable standalone server: one listener, legacy settings and persisted data.
Build server and agent-process-host first. Never points at an existing instance.
"""
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / '.build'
BUILD.mkdir(exist_ok=True)

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None

opener = urllib.request.build_opener(NoRedirect)
with tempfile.TemporaryDirectory(prefix='local247-instance-', dir=BUILD) as directory:
    instance = Path(directory)
    # Occupied legacy port must neither prevent startup nor be used as a listener.
    with socket.socket() as occupied:
        occupied.bind(('127.0.0.1', 0))
        occupied.listen()
        legacy_port = occupied.getsockname()[1]
        env = {key: os.environ[key] for key in ['PATH', 'HOME', 'USER', 'LANG', 'RUSTUP_HOME', 'CARGO_HOME', 'PLAYWRIGHT_BROWSERS_PATH'] if key in os.environ}
        env.update(LVK_INSTANCE_DIR=str(instance), LVK_ACCESS_DEPLOYMENT='local', LVK_AUTH_MODE='local',
                   LVK_PUBLIC_URL='', HOST='127.0.0.1', PORT='0', BACKEND_PORT='0',
                   PREVIEW_PROXY_PORT=str(legacy_port), LVK_PREVIEW_DOMAIN='invalid old value',
                   VK_PREVIEW_DOMAIN='invalid old value', VK_PREVIEW_DISABLE_NAV_SCRIPT='1',
                   VIBE_KANBAN_AGENT_PROCESS_HOST=str(ROOT / 'target/debug/agent-process-host'),
                   DISABLE_WORKTREE_CLEANUP='1', BROWSER='true', RUST_LOG='info')
        def start(number):
            log = (instance / f'server-{number}.log').open('w+')
            process = subprocess.Popen([str(ROOT / 'target/debug/server')], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 90
            try:
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise AssertionError('isolated server exited: ' + (instance / f'server-{number}.log').read_text())
                    port_file = instance / 'ports/vibe-kanban.port'
                    if port_file.exists():
                        ports = json.loads(port_file.read_text())
                        assert set(ports) == {'main_port'}, ports
                        base = f'http://127.0.0.1:{ports["main_port"]}'
                        try:
                            with opener.open(base + '/api/health', timeout=1) as response:
                                if response.status == 200:
                                    own = set()
                                    for fd in Path(f'/proc/{process.pid}/fd').iterdir():
                                        try:
                                            link = fd.readlink().as_posix()
                                            if link.startswith('socket:['): own.add(link[8:-1])
                                        except FileNotFoundError:
                                            pass
                                    listeners = []
                                    for table in ['tcp','tcp6']:
                                        for row in Path(f'/proc/{process.pid}/net/{table}').read_text().splitlines()[1:]:
                                            fields = row.split()
                                            if fields[3] == '0A' and fields[9] in own:
                                                listeners.append(int(fields[1].split(':')[1],16))
                                    assert listeners == [ports['main_port']], listeners
                                    return process, log, base
                        except (OSError, urllib.error.URLError):
                            pass
                    time.sleep(.05)
                raise AssertionError('startup timeout')
            except BaseException:
                stop(process, log)
                raise
        def stop(process, log):
            process.send_signal(signal.SIGTERM)
            try:
                code = process.wait(timeout=30)
                assert code == 0, code
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
                log.close()
        def get(base, path):
            with opener.open(base + path, timeout=5) as response:
                return json.load(response)
        checksum = hashlib.sha256((ROOT / 'crates/db/migrations/20261015010000_preview_access.sql').read_bytes()).hexdigest()
        process, log, base = start(1)
        try:
            info = get(base, '/api/info')['data']
            assert 'preview_proxy_port' not in info and 'preview_proxy_domain' not in info
            for path in ['/api/preview/5173', '/api/access/preview/authorize']:
                try:
                    opener.open(base + path, timeout=5)
                    raise AssertionError('retired API still responds: ' + path)
                except urllib.error.HTTPError as error:
                    assert error.code == 404, (path, error.code)
        finally:
            stop(process, log)
        database = instance / 'data/db.v2.sqlite'
        scratch_id = uuid.uuid4()
        payload = {'type':'PREVIEW_SETTINGS','data':{'url':'http://localhost:5173','screen_size':'mobile'}}
        with sqlite3.connect(database) as db:
            migration = db.execute('SELECT checksum FROM _sqlx_migrations WHERE version=20261015010000').fetchone()
            assert migration is not None
            previous_migrations = db.execute('SELECT version, checksum FROM _sqlx_migrations ORDER BY version').fetchall()
            db.execute("INSERT INTO scratch(id,scratch_type,payload,created_at,updated_at) VALUES(?,?,?,'2026-10-27T00:00:00Z','2026-10-27T00:00:00Z')",(scratch_id.bytes,'PREVIEW_SETTINGS',json.dumps(payload)))
            db.execute("INSERT INTO access_sessions(hash,kind,csrf,expires,touched,revision) VALUES('historical-session','web','fixture',4102444800,4102444800,1)")
            db.execute("INSERT INTO access_preview_sessions(hash,origin,session_hash,expires) VALUES('historical-preview','https://old.example.test','historical-session',4102444800)")
        (instance / 'ports/vibe-kanban.port').unlink()
        env['PREVIEW_PROXY_PORT'] = '0'
        process, log, base = start(2)
        try:
            old = get(base, f'/api/scratch/PREVIEW_SETTINGS/{scratch_id}')['data']
            assert old['payload']['data']['url'] == payload['data']['url'], old
            assert old['payload']['data']['screen_size'] == 'mobile'
            subprocess.run(['node', 'tests/embedded-browser-removal/smoke.mjs'], cwd=ROOT, env={**env, 'LVK_TEST_BASE_URL':base}, check=True, timeout=60)
            # Generic listing must also decode the retired payload.
            assert any(item['payload']['type']=='PREVIEW_SETTINGS' for item in get(base,'/api/scratch')['data'])
        finally:
            stop(process, log)
        with sqlite3.connect(database) as db:
            assert db.execute('SELECT version, checksum FROM _sqlx_migrations ORDER BY version').fetchall() == previous_migrations
            assert db.execute("SELECT kind FROM access_sessions WHERE hash='historical-session'").fetchone() == ('web',)
            assert db.execute("SELECT origin FROM access_preview_sessions WHERE hash='historical-preview'").fetchone() == ('https://old.example.test',)
        logs = (instance / 'server-2.log').read_text()
        for key in ['PREVIEW_PROXY_PORT','LVK_PREVIEW_DOMAIN','VK_PREVIEW_DOMAIN','VK_PREVIEW_DISABLE_NAV_SCRIPT']:
            assert key in logs and 'retired and ignored' in logs
        assert hashlib.sha256((ROOT / 'crates/db/migrations/20261015010000_preview_access.sql').read_bytes()).hexdigest() == checksum
        print('PASS isolated standalone start/stop twice, occupied legacy port ignored, retired APIs absent, new info/port-file, retained migrations/sessions and old Scratch API/list decode; temporary instance removed')
