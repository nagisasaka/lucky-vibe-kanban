#!/usr/bin/env python3
"""Foreground supervisor, invoked only by the Workspace managed DevServer."""
import base64
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import time
import urllib.request

repo = Path(__file__).resolve().parents[1]
state = repo / '.build/design-preview'
state.mkdir(parents=True, exist_ok=True, mode=0o700)
instance = state / 'instance'
instance.mkdir(exist_ok=True, mode=0o700)
assert instance.resolve().is_relative_to(state.resolve())
binary = Path('/var/tmp/lvk-instances/dev/releases/d21d587f-8e61-4ae1-bde8-d745c34bec36/server')
with binary.open('rb') as stream:
    assert hashlib.file_digest(stream, 'sha256').hexdigest() == '5adaed1969154330b563a62dd03d76ff2e97731dbe8fa1cfccbf4daba553f8ae', 'Backend artifact changed; deliberate compatibility review required'
for port in (18444, 18446):
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(('127.0.0.1', port))
env = {k: os.environ[k] for k in ('HOME', 'PATH', 'LANG') if k in os.environ}
env.update({
    'LVK_INSTANCE_DIR': str(instance), 'HOST': '127.0.0.1', 'PORT': '18446',
    'BACKEND_PORT': '18446',
    'LVK_ACCESS_DEPLOYMENT': 'public', 'LVK_AUTH_MODE': 'basic',
    'LVK_PUBLIC_URL': 'https://preview2.luckyvibekanban.com',
    'DISABLE_WORKTREE_CLEANUP': '1', 'RUST_LOG': 'warn',
})
credentials_path = state / 'credentials.json'
if not credentials_path.exists():
    with credentials_path.open('x') as stream:
        os.chmod(credentials_path, 0o600)
        json.dump({'username': 'preview', 'password': secrets.token_urlsafe(18)}, stream)
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None
opener = urllib.request.build_opener(NoRedirect)
def api(path, data=None, extra=None):
    headers = {'Host': 'preview2.luckyvibekanban.com', 'Origin': 'https://preview2.luckyvibekanban.com'}
    headers.update(extra or {})
    if data is not None:
        headers['Content-Type'] = 'application/json'
    request = urllib.request.Request('http://127.0.0.1:18446/api/access/' + path,
        data=None if data is None else json.dumps(data).encode(), headers=headers)
    with opener.open(request, timeout=10) as response:
        return json.load(response), response.headers
children = []
stopping = False
def stop(*_):
    global stopping
    stopping = True
for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, stop)
try:
    with (state / 'backend.log').open('a') as log:
        children.append(subprocess.Popen([str(binary)], cwd=repo, env=env, stdout=log, stderr=log))
    for _ in range(150):
        if stopping or children[0].poll() is not None:
            raise RuntimeError('Backend stopped before readiness')
        try:
            status, _ = api('status')
            break
        except (OSError, urllib.error.URLError):
            time.sleep(0.2)
    else:
        raise RuntimeError('Backend readiness timeout')
    if status['mode'] == 'locked':
        with (state / 'setup.log').open('a') as log:
            command = subprocess.run([str(binary), 'setup-token'], env=env, cwd=repo, capture_output=True, check=True)
            # Setup token is consumed in memory only, never logged.
        result, headers = api('setup', {'key': command.stdout.decode().strip()}, {'x-lvk-csrf': 'setup'})
        cookie = headers['Set-Cookie'].split(';')[0]
        api('basic', json.loads(credentials_path.read_text()), {'x-lvk-csrf': result['csrf'], 'Cookie': cookie})
    else:
        assert status['mode'] == 'basic', 'Refuse to replace an existing authentication configuration'
    children.append(subprocess.Popen(['node', 'node_modules/vite/bin/vite.js', '--config', str(repo / '.lvk-preview/vite.config.mjs')], cwd=repo / 'packages/local-web', env=env))
    print('Isolated preview backend and Vite started under Workspace DevServer', flush=True)
    while not stopping and all(child.poll() is None for child in children):
        time.sleep(0.2)
finally:
    for child in children:
        if child.poll() is None:
            child.terminate()
    for child in children:
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
if not stopping:
    raise SystemExit(next((child.returncode for child in children if child.returncode), 1))
