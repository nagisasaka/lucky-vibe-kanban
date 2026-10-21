"""Run the opt-in browser test in fresh disposable local state (no live LVK)."""
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / '.build' / 'integration-review'
OUTPUT.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='local9-scenario-', dir=OUTPUT) as instance:
    env = dict(os.environ, LVK_INSTANCE_DIR=instance, HOST='127.0.0.1',
               LVK_ACCESS_DEPLOYMENT='local', LVK_AUTH_MODE='local',
               DISABLE_WORKTREE_CLEANUP='1', RUST_LOG='warn')
    for key in ('LVK_PUBLIC_URL', 'LVK_MCP_CONFIG', 'VIBE_BACKEND_URL',
                'VK_ALLOWED_ORIGINS', 'LVK_INTEGRATION_REAL_PROVIDER'):
        env.pop(key, None)
    command = ['cargo', 'test', '-j', '1', '-p', 'server', '--lib',
               'isolated_review_browser', '--', '--ignored', '--nocapture',
               '--test-threads=1']
    with (OUTPUT / 'e2e.log').open('w') as log:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        try:
            result = child.wait(timeout=3600)
        finally:
            # Contain cargo, test server, Node and browser children on failures too.
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            # Let Node's SIGTERM handler close Chromium before disposing state.
            for _ in range(100):
                try:
                    os.killpg(child.pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.1)
            else:
                os.killpg(child.pid, signal.SIGKILL)
    print((OUTPUT / 'e2e.log').read_text()[-12000:])
    raise SystemExit(result)
