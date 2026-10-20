"""Run the built checkout on private loopback state, test, and settle its processes."""
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import subprocess
import time
import urllib.request
from uuid import uuid4

root = Path(__file__).resolve().parents[2]
evidence = root / 'test-results/local21' / ('e2e-' + str(uuid4()))
evidence.mkdir(parents=True, exist_ok=True)
instance = evidence / 'instance'
instance.mkdir(exist_ok=True)
(instance / 'tmp').mkdir(exist_ok=True)
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    port = listener.getsockname()[1]
base = f'http://127.0.0.1:{port}'
env = dict(os.environ, LVK_INSTANCE_DIR=str(instance), LVK_ACCESS_DEPLOYMENT='local',
           LVK_AUTH_MODE='', LVK_PUBLIC_URL='', HOST='127.0.0.1', PORT='',
           BACKEND_PORT=str(port), PREVIEW_PROXY_PORT='0', VK_ALLOWED_ORIGINS=base,
           VK_PREVIEW_DOMAIN='', VIBE_KANBAN_AGENT_PROCESS_HOST=str(root / 'target/debug/agent-process-host'),
           TMPDIR=str(instance / 'tmp'), DISABLE_WORKTREE_CLEANUP='1', BROWSER='true',
           LVK_E2E_BASE_URL=base, LVK_E2E_INSTANCE=str(instance), LVK_E2E_EVIDENCE=str(evidence))
(evidence / 'instance.json').write_text(json.dumps({'base_url': base, 'instance': str(instance)}, indent=2))
with (evidence / 'server.log').open('w') as log:
    server = subprocess.Popen([str(root / 'target/debug/server')], cwd=root, env=env,
                              stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        for _ in range(120):
            if server.poll() is not None:
                raise RuntimeError(f'Isolated server exited {server.returncode}; see server.log')
            try:
                with urllib.request.urlopen(base + '/api/info', timeout=1) as response:
                    if response.status == 200:
                        break
            except (OSError, urllib.error.URLError):
                time.sleep(0.5)
        else:
            raise RuntimeError('Isolated backend health timeout')
        print('ISOLATED_BASE_URL', base, 'EVIDENCE', evidence, flush=True)
        result = subprocess.run(['pnpm', 'exec', 'playwright', 'test', '-c', 'tests/chat/isolated.config.ts'], cwd=root, env=env)
    finally:
        if server.poll() is None:
            os.killpg(server.pid, signal.SIGTERM)
            try:
                server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid, signal.SIGKILL)
                server.wait()
        # Owned child process hosts share the group; settle any remaining children.
        try:
            os.killpg(server.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        with socket.socket() as probe:
            probe.settimeout(1)
            idle = probe.connect_ex(('127.0.0.1', port)) != 0
        registry_path = instance / 'data/runtime/agent-process-registry.json'
        remaining = json.loads(registry_path.read_text()).get('processes', []) if registry_path.exists() else []
        (evidence / 'cleanup.json').write_text(json.dumps({'server_exit': server.returncode, 'port_closed': idle, 'registered_processes_remaining': len(remaining)}, indent=2))
        if remaining:
            raise RuntimeError('Owned instance still has registered agent processes; inspect before cleanup')
        if not idle:
            raise RuntimeError('Owned loopback listener did not settle')
        shutil.rmtree(instance / 'tmp', ignore_errors=False)
raise SystemExit(result.returncode)
