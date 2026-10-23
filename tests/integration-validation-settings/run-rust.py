"""Run only LOCAL130 Rust checks with disposable state and retained evidence.

Coordinate the Rust slot before invoking this script. Never points at live state.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'test-results' / 'integration-validation-settings' / 'rust'
OUTPUT.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, CARGO_BUILD_JOBS='2')
if not ENV.get('RUSTC_WRAPPER') and shutil.which('sccache'):
    ENV['RUSTC_WRAPPER'] = shutil.which('sccache')
if (ROOT / '.lvk-shared/cache').exists():
    ENV['SCCACHE_DIR'] = str((ROOT / '.lvk-shared/cache/sccache').resolve())
STAGES = {
    'check': ['cargo', 'check', '-p', 'server'],
    'unit': ['cargo', 'test', '-p', 'server', '--lib', 'routes::integration_validation::tests'],
    'frozen': ['cargo', 'test', '-p', 'server', '--lib', 'mechanical_validation_is_frozen_from_target_and_skip_never_reads_or_runs_commands'],
    'http': ['cargo', 'test', '-p', 'server', '--lib', 'isolated_validation_settings_http_and_executor', '--', '--ignored', '--nocapture', '--test-threads=1'],
}


def run(name, command):
    start = time.time()
    with tempfile.TemporaryDirectory(prefix='local130-validation-', dir=OUTPUT) as instance:
        env = dict(ENV, LVK_INSTANCE_DIR=instance, HOST='127.0.0.1',
                   LVK_ACCESS_DEPLOYMENT='local', LVK_AUTH_MODE='local',
                   DISABLE_WORKTREE_CLEANUP='1', RUST_LOG='warn')
        for key in ('LVK_PUBLIC_URL', 'LVK_MCP_CONFIG', 'VIBE_BACKEND_URL',
                    'VK_ALLOWED_ORIGINS', 'LVK_INTEGRATION_REAL_PROVIDER'):
            env.pop(key, None)
        with (OUTPUT / f'{name}.log').open('w') as log:
            child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log,
                                     stderr=subprocess.STDOUT, start_new_session=True)
            try:
                code = child.wait(timeout=3600)
            finally:
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                for _ in range(100):
                    try:
                        os.killpg(child.pid, 0)
                    except ProcessLookupError:
                        break
                    time.sleep(0.1)
                else:
                    os.killpg(child.pid, signal.SIGKILL)
                    raise RuntimeError('Process group did not settle; inspect retained logs')
        result = dict(command=command, code=code, started=start,
                      finished=time.time(), elapsed_seconds=round(time.time()-start, 2))
        (OUTPUT / f'{name}.json').write_text(json.dumps(result, indent=2)+'\n')
        print((OUTPUT / f'{name}.log').read_text()[-8000:], flush=True)
        print(json.dumps(result), flush=True)
        if code:
            raise SystemExit(code)


if __name__ == '__main__':
    import sys
    for stage in sys.argv[1:] or STAGES:
        run(stage, STAGES[stage])
