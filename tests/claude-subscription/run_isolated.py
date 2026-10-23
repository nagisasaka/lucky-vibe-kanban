#!/usr/bin/env python3
"""Run the changed server/host and Chromium with disposable instance state.
Build binaries and install pnpm dependencies first. No live instance is used.
"""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
evidence_root = ROOT / '.build/claude-subscription'
evidence_root.mkdir(parents=True, exist_ok=True)
EVIDENCE = evidence_root / f'e2e-{time.time_ns()}'
EVIDENCE.mkdir()
print(f'Evidence: {EVIDENCE}', flush=True)


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def wait_http(port, path, process):
    for _ in range(120):
        if process.poll() is not None:
            raise RuntimeError(f'Process exited during startup: {process.returncode}')
        try:
            with urllib.request.urlopen(f'http://127.0.0.1:{port}{path}', timeout=1):
                return
        except (OSError, urllib.error.URLError):
            time.sleep(.5)
    raise RuntimeError(f'Startup timeout on port {port}')


def owned_pids(instance):
    marker = f'LVK_INSTANCE_DIR={instance}'.encode()
    found = []
    for entry in Path('/proc').iterdir():
        if entry.name.isdigit():
            try:
                if marker in (entry / 'environ').read_bytes().split(b'\0'):
                    found.append(int(entry.name))
            except (OSError, PermissionError):
                pass
    return found


with tempfile.TemporaryDirectory(prefix='lvk-claude-subscription-') as temporary:
    temp = Path(temporary)
    instance = temp / 'instance'
    for path in [instance / 'data', temp / 'trace', temp / 'workspaces', temp / 'codex', temp / 'claude-config', temp / 'bin']:
        path.mkdir(parents=True)
    backend, frontend, proxy = free_port(), free_port(), free_port()
    assert len({backend, frontend, proxy}) == 3
    env = {key: value for key, value in os.environ.items() if not key.startswith(('LVK_', 'VK_', 'VIBE_KANBAN_', 'ANTHROPIC_', 'CLAUDE_')) and key not in ('DATABASE_URL', 'BACKEND_PORT', 'FRONTEND_PORT', 'PREVIEW_PROXY_PORT', 'HOST')}
    env.update({
        'LVK_INSTANCE_DIR': str(instance), 'LVK_ACCESS_DEPLOYMENT': 'local', 'LVK_AUTH_MODE': 'local',
        'VIBE_KANBAN_AGENT_PROCESS_HOST': str(ROOT / 'target/debug/agent-process-host'),
        'HOST': '127.0.0.1', 'BACKEND_PORT': str(backend), 'FRONTEND_PORT': str(frontend),
        'PREVIEW_PROXY_PORT': str(proxy), 'CODEX_HOME': str(temp / 'codex'),
        'VK_ALLOWED_ORIGINS': f'http://127.0.0.1:{frontend}', 'VITE_OPEN': 'false',
        'LVK_CLAUDE_FIXTURE_TRACE': str(temp / 'trace'), 'LVK_CLAUDE_TEST_ROOT': str(temp / 'workspaces'),
        'LVK_E2E_BASE_URL': f'http://127.0.0.1:{frontend}', 'DISABLE_WORKTREE_CLEANUP': '1', 'TOKIO_WORKER_THREADS': '2', 'GOMAXPROCS': '2',
    })
    (temp / 'bin/claude').symlink_to(ROOT / 'tests/claude-subscription/cli_fixture.py')
    (temp / 'state.json').write_text('{}')
    env.update({'PATH': str(temp / 'bin') + os.pathsep + env['PATH'],
                'CLAUDE_CONFIG_DIR': str(temp / 'claude-config'),
                'LVK_CLAUDE_FIXTURE_STATE': str(temp / 'state.json')})
    profile_env = {key: env[key] for key in ('PATH', 'CLAUDE_CONFIG_DIR', 'LVK_CLAUDE_FIXTURE_STATE', 'LVK_CLAUDE_FIXTURE_TRACE')}
    profiles = {'DEFAULT': {'CLAUDE_CODE': {'approvals': True, 'env': profile_env}},
                'SUBSCRIPTION': {'CLAUDE_CODE': {'approvals': True, 'require_subscription_login': True, 'env': profile_env}},
                'CONFLICT': {'CLAUDE_CODE': {'require_subscription_login': True, 'env': {**profile_env, 'ANTHROPIC_API_KEY': 'offline-conflict-sentinel'}}}}
    (instance / 'data/profiles.json').write_text(json.dumps({'executors': {'CLAUDE_CODE': profiles}}))
    processes = []
    logs = []
    code = 1
    try:
        for name, command in [('server', [str(ROOT / 'target/debug/server')]),
                              ('vite', ['pnpm', '--dir', 'packages/local-web', 'exec', 'vite', '--host', '127.0.0.1', '--port', str(frontend), '--strictPort'])]:
            log = open(EVIDENCE / f'{name}.log', 'w')
            logs.append(log)
            processes.append(subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True))
            wait_http(backend if name == 'server' else frontend, '/api/health' if name == 'server' else '/', processes[-1])
        with open(EVIDENCE / 'browser.log', 'w') as log:
            code = subprocess.call(['pnpm', 'exec', 'playwright', 'test', '-c', 'tests/claude-subscription/playwright.config.ts'], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    finally:
        # Include independently sessioned process hosts and their provider children.
        owned = owned_pids(instance)
        for pid in owned:
            try: os.kill(pid, signal.SIGTERM)
            except ProcessLookupError: pass
        for process in processes:
            try: process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
        for _ in range(50):
            survivors = owned_pids(instance)
            if not survivors: break
            time.sleep(.1)
        for pid in owned_pids(instance):
            try: os.kill(pid, signal.SIGKILL)
            except ProcessLookupError: pass
        time.sleep(.2)
        remaining = owned_pids(instance)
        listeners = []
        for port in [backend, frontend, proxy]:
            with socket.socket() as s:
                if s.connect_ex(('127.0.0.1', port)) == 0: listeners.append(port)
        cleanup = {'owned_pids': owned, 'remaining_pids': remaining, 'ports': [backend, frontend, proxy], 'remaining_listeners': listeners}
        (EVIDENCE / 'cleanup.json').write_text(json.dumps(cleanup, indent=2))
        with open(EVIDENCE / 'native-fixture.jsonl', 'w') as output:
            for trace in sorted((temp / 'trace').glob('*.jsonl')):
                output.write(trace.read_text())
        for log in logs: log.close()
        print(json.dumps({'exit_code': code, 'cleanup': cleanup}))
        if remaining or listeners: raise RuntimeError('Owned process cleanup incomplete')
    raise SystemExit(code)
