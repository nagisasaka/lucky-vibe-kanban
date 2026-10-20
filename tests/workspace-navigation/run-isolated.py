#!/usr/bin/env python3
"""自Workspaceでビルド済みの実アプリを一時DB・loopbackで検証する。"""
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
EVIDENCE = ROOT / 'test-results/local31'
EVIDENCE.mkdir(parents=True, exist_ok=True)


def stop(process):
    if process is None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)
    # Browser helpers may outlive the Playwright CLI after a failed test.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def own_browser_pids(instance):
    # Playwright creates Chromium profiles under this run's TMPDIR.
    owned = []
    for entry in Path('/proc').glob('[0-9]*'):
        try:
            name = (entry / 'comm').read_text().strip()
            if 'chrom' not in name:
                continue
            if str(instance).encode() in (entry / 'cmdline').read_bytes():
                owned.append(int(entry.name))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    return owned


def interrupted(signum, frame):
    raise SystemExit(128 + signum)


signal.signal(signal.SIGTERM, interrupted)
signal.signal(signal.SIGINT, interrupted)
server = browser = None
ports = []
with tempfile.TemporaryDirectory(prefix='lvk-local31-', dir='/tmp') as temporary:
    instance = Path(temporary)
    fixture = instance / 'fixture-directory'
    fixture.mkdir()
    (fixture / 'child').mkdir()
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    env = dict(os.environ)
    env.update(
        LVK_INSTANCE_DIR=str(instance), TMPDIR=str(instance), LVK_ACCESS_DEPLOYMENT='local',
        LVK_AUTH_MODE='', LVK_PUBLIC_URL='', HOST='127.0.0.1',
        BACKEND_PORT=str(port), PORT='', PREVIEW_PROXY_PORT='0',
        VK_ALLOWED_ORIGINS=f'http://127.0.0.1:{port}', VK_PREVIEW_DOMAIN='',
        DISABLE_WORKTREE_CLEANUP='1', BROWSER='true', RUST_LOG='warn',
        VIBE_KANBAN_AGENT_PROCESS_HOST=str(ROOT / 'target/debug/agent-process-host'),
        LVK_E2E_BASE_URL=f'http://127.0.0.1:{port}', LVK_E2E_DIRECTORY=str(fixture),
    )
    env.pop('DATABASE_URL', None)
    result = 1
    try:
        with (EVIDENCE / 'server.log').open('w') as server_log:
            server = subprocess.Popen([str(ROOT / 'target/debug/server')], cwd=ROOT,
                                      env=env, stdout=server_log, stderr=subprocess.STDOUT,
                                      start_new_session=True)
            for _ in range(120):
                if server.poll() is not None:
                    raise RuntimeError('試験サーバーの起動に失敗。server.logを確認')
                try:
                    with urllib.request.urlopen(env['LVK_E2E_BASE_URL'] + '/api/info', timeout=1) as response:
                        if response.status == 200:
                            break
                except (OSError, urllib.error.URLError):
                    time.sleep(1)
            else:
                raise TimeoutError('試験サーバー起動タイムアウト')
            port_info = json.loads((instance / 'ports/vibe-kanban.port').read_text())
            ports = [port_info['main_port'], port_info['preview_proxy_port']]
            (EVIDENCE / 'instance.json').write_text(json.dumps({
                'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'instance': str(instance), 'ports': port_info,
                'source': str(ROOT), 'fixture': '独立一時DBの設定と空Workspace。API mockなし。Agent実行なし。',
            }, ensure_ascii=False, indent=2))
            with (EVIDENCE / 'e2e.log').open('w') as log:
                browser = subprocess.Popen(['pnpm', 'exec', 'playwright', 'test', '-c',
                                            'tests/workspace-navigation/playwright.config.ts'],
                                           cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                                           start_new_session=True)
                result = browser.wait(timeout=180)
    finally:
        stop(browser)
        stop(server)
        remaining_browsers = own_browser_pids(instance)
        for pid in remaining_browsers:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        for _ in range(50):
            if not own_browser_pids(instance):
                break
            time.sleep(0.1)
        if own_browser_pids(instance):
            raise RuntimeError('試験用Chromiumがまだ残っています')
        for port in ports:
            with socket.socket() as client:
                if client.connect_ex(('127.0.0.1', port)) == 0:
                    raise RuntimeError(f'試験ポート{port}がまだ開いています')
        (EVIDENCE / 'cleanup.json').write_text(json.dumps({
            'server_exit': server.returncode if server else None,
            'playwright_exit': browser.returncode if browser else None,
            'ports_closed': ports,
            'remaining_browser_processes': own_browser_pids(instance),
            'instance': str(instance),
        }, indent=2))
cleanup = json.loads((EVIDENCE / 'cleanup.json').read_text())
cleanup['temporary_instance_removed'] = not instance.exists()
(EVIDENCE / 'cleanup.json').write_text(json.dumps(cleanup, indent=2))
raise SystemExit(result)
