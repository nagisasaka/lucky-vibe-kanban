#!/usr/bin/env python3
"""Workspace-owned foreground preview. Invoke via LVK managed DevServer only."""
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time

repo = Path(__file__).resolve().parents[1]
config = json.loads((repo / '.build/ui4-preview/config.json').read_text())
instance = (repo / config['instance']).resolve()
assert instance.is_relative_to(repo / '.build/ui4-preview')
assert (instance / 'data/db.v2.sqlite').is_file()
server = repo / 'target/debug/server'
assert server.is_file()
with server.open('rb') as binary:
    assert hashlib.file_digest(binary, 'sha256').hexdigest() == config['binary_sha256'], 'Rebuilt backend needs deliberate verification/config refresh'
subprocess.run(['git', 'diff', '--exit-code', config['backend_source_revision'], '--', 'crates', 'Cargo.toml', 'Cargo.lock'], cwd=repo, check=True, stdout=subprocess.DEVNULL)
for port in (18444, 18446):
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(('127.0.0.1', port))  # Refuse occupants; never stop them.
env = {k: os.environ[k] for k in ('HOME', 'PATH', 'LANG') if k in os.environ}
fixture_bin = repo / '.build/ui4-preview/bin'
assert all((fixture_bin / name).is_file() for name in ('codex', 'claude'))
env['PATH'] = str(fixture_bin) + ':' + env['PATH']
env.update({
    'LVK_INSTANCE_DIR': str(instance), 'HOST': '127.0.0.1',
    'PORT': '18446', 'BACKEND_PORT': '18446', 'PREVIEW_PROXY_PORT': '0',
    'LVK_ACCESS_DEPLOYMENT': 'public', 'LVK_AUTH_MODE': 'basic',
    'LVK_PUBLIC_URL': 'https://preview2.luckyvibekanban.com',
    'VITE_KANBAN_MOTION': os.environ.get('VITE_KANBAN_MOTION', 'crisp'),
    'DISABLE_WORKTREE_CLEANUP': '1',
})
children = []
stop = False

def request_stop(_signal, _frame):
    global stop
    stop = True

for sig in (signal.SIGINT, signal.SIGTERM):
    signal.signal(sig, request_stop)
try:
    children.append(subprocess.Popen([str(server)], cwd=repo, env=env))
    children.append(subprocess.Popen(['node', 'node_modules/vite/bin/vite.js', '--config', str(repo / '.lvk-preview/vite.config.mjs')], cwd=repo / 'packages/local-web', env=env))
    while not stop and all(p.poll() is None for p in children):
        time.sleep(0.2)
finally:
    for child in children:
        if child.poll() is None:
            child.terminate()
    for child in children:
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
sys.exit(0 if stop else next((p.returncode for p in children if p.returncode), 1))
