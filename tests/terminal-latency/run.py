"""Run the built workspace app and Chromium against task-owned loopback state.

Build first: pnpm --dir packages/local-web run build
             cargo build --bin server --bin agent-process-host
Run: python3 tests/terminal-latency/run.py
Only the Codex subprocess is replaced with a deterministic protocol fixture.
"""
import json
import hashlib
import base64
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix="lvk-terminal-latency-"))
evidence = repo / "test-results/terminal-latency-instance"
evidence.mkdir(parents=True, exist_ok=True)
instance = root / "instance"
(instance / "data").mkdir(parents=True)
fixtures = root / "fixtures"
fixtures.mkdir()
(instance / "data/profiles.json").write_text(json.dumps({"executors": {"CODEX": {"DEFAULT": {"CODEX": {"sandbox": "danger-full-access", "base_command_override": f"node {repo / 'tests/terminal-latency/codex-fixture.mjs'}"}}}}}))
env = {**os.environ, "LVK_INSTANCE_DIR": str(instance), "VIBE_KANBAN_AGENT_PROCESS_HOST": str(repo / "target/debug/agent-process-host"), "HOST": "127.0.0.1", "BACKEND_PORT": "0", "PREVIEW_PROXY_PORT": "0", "LVK_ACCESS_DEPLOYMENT": "local", "LVK_AUTH_MODE": "local", "LVK_PUBLIC_URL": "", "DISABLE_WORKTREE_CLEANUP": "1", "RUST_LOG": "info"}
server = None
provenance = {
    'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
    'source_sha256': hashlib.sha256((repo / 'crates/local-deployment/src/process_host.rs').read_bytes()).hexdigest(),
    'binaries': {name: {'path': str(repo / f'target/debug/{name}'), 'sha256': hashlib.file_digest((repo / f'target/debug/{name}').open('rb'), 'sha256').hexdigest()} for name in ('server', 'agent-process-host')},
}
(evidence / 'provenance.json').write_text(json.dumps(provenance, indent=2))

def owned_processes():
    found = []
    for path in Path('/proc').glob('[0-9]*/environ'):
        try:
            if f"LVK_INSTANCE_DIR={instance}".encode() in path.read_bytes().split(b'\0'):
                pid = int(path.parent.name)
                found.append(pid)
        except (OSError, PermissionError):
            pass
    return found

try:
    with (evidence / "server.log").open('w') as log:
        server = subprocess.Popen([str(repo / 'target/debug/server')], cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic() + 120
        base = None
        while time.monotonic() < deadline:
            if server.poll() is not None:
                raise RuntimeError(f"Server exited {server.returncode}; see {evidence / 'server.log'}")
            match = re.search(r'Main server on :(\d+)', (evidence / 'server.log').read_text())
            if match:
                startup = (evidence / 'server.log').read_text()
                if 'Loaded user profile overrides' not in startup or 'Failed to parse user profiles' in startup:
                    raise RuntimeError('Fixture launch profile was not loaded; refusing provider execution')
                base = f"http://127.0.0.1:{match[1]}"
                with urllib.request.urlopen(base + '/api/info') as response:
                    info = json.load(response)
                    assert info['success']
                with urllib.request.urlopen(base + '/api/access/status') as response:
                    csrf = json.load(response)['csrf']
                config = info['data']['config']
                config.update(onboarding_acknowledged=True, disclaimer_acknowledged=True, analytics_enabled=False, showcases={"seen_features": ["workspaces-guide"]})
                req = urllib.request.Request(base + '/api/config', method='PUT', data=json.dumps(config).encode(), headers={'Content-Type': 'application/json', 'Origin': base, 'X-LVK-CSRF': csrf})
                with urllib.request.urlopen(req) as response:
                    assert json.load(response)['success']
                break
            time.sleep(.1)  # readiness polling in the test runner, not product logic
        if not base:
            raise RuntimeError('Isolated server did not become ready')
        print(f"Isolated backend: {base}; instance: {instance}", flush=True)
        test_env = {**os.environ, 'LVK_E2E_BASE_URL': base, 'LVK_LATENCY_FIXTURE_ROOT': str(fixtures), 'LVK_EXPECTED_PROCESS_HOST': str(repo / 'target/debug/agent-process-host'), 'LVK_E2E_CHROMIUM': os.environ.get('LVK_E2E_CHROMIUM', '/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome')}
        result = subprocess.run(['pnpm', 'exec', 'playwright', 'test', '-c', 'tests/terminal-latency/playwright.config.ts', *sys.argv[1:]], cwd=repo, env=test_env)
        if result.returncode:
            raise RuntimeError(f'Browser regression failed ({result.returncode})')
finally:
    # Unblock any provider left waiting after a browser assertion failed.
    for fixture in fixtures.iterdir():
        (fixture / 'release').write_text('completed')
    if server is not None:
        # Give host journal ACK/normal cleanup a chance to settle first.
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            if all(pid == server.pid for pid in owned_processes()):
                break
            time.sleep(.2)
        server.send_signal(signal.SIGTERM) if server.poll() is None else None
        try:
            server.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(server.pid, signal.SIGKILL)
            server.wait()
    remaining = owned_processes()
    if remaining:
        raise RuntimeError(f'Own instance processes remain {remaining}; retained {root} for recovery')
    # Keep diagnostic journals, not the disposable DB/runtime or binaries.
    timings = []
    for path in instance.rglob('*.jsonl'):
        if 'host-events' in str(path):
            shutil.copy2(path, evidence / path.name)
            for line in path.read_text().splitlines():
                for event in json.loads(line).get('events', []):
                    payload = event.get('payload', {})
                    if payload.get('type') == 'terminal':
                        timings.append({'stage': 'host_terminal', 'at': event['timestamp'], 'journal': path.name, 'data': payload['data']})
        elif path.name == 'frames.jsonl':
            for line in path.read_text().splitlines():
                frame = json.loads(line)
                raw = base64.b64decode(frame['payload_base64'])
                try:
                    native = json.loads(raw)
                except (ValueError, UnicodeDecodeError):
                    continue
                if native.get('method') == 'turn/completed':
                    timings.append({'stage': 'native_terminal_audited', 'at': frame['timestamp'], 'stream': str(path.relative_to(instance)), 'params': native.get('params')})
    (evidence / 'timings.json').write_text(json.dumps(timings, indent=2))
    for fixture in fixtures.iterdir():
        observation = fixture / 'observations.jsonl'
        if observation.exists():
            shutil.copy2(observation, evidence / f'{fixture.name}.jsonl')
    shutil.rmtree(root)
    print('Verified isolated instance processes stopped; removed disposable state', flush=True)
