"""Keep a real /api/events SSE open across SIGTERM in disposable instances.

Build server and `cargo test -p server --test shutdown --no-run` through
scripts/dev-cargo.mjs first. Pass the emitted test executable with
--library-test-binary to exercise ServerHandle::serve as well as main.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def run(mode, command, expect_hang, close_sse):
    with tempfile.TemporaryDirectory(prefix='local247-shutdown-', dir=ROOT / '.build') as directory:
        instance = Path(directory)
        (instance / 'shutdown-test-marker').write_text('isolated shutdown test')
        env = {key: os.environ[key] for key in ['PATH', 'HOME', 'USER', 'LANG', 'RUSTUP_HOME', 'CARGO_HOME'] if key in os.environ}
        env.update(LVK_INSTANCE_DIR=directory, LVK_SHUTDOWN_TEST_INSTANCE=directory,
                   LVK_ACCESS_DEPLOYMENT='local', LVK_AUTH_MODE='local',
                   LVK_PUBLIC_URL='', HOST='127.0.0.1', PORT='0', BACKEND_PORT='0',
                   DISABLE_WORKTREE_CLEANUP='1', BROWSER='true', RUST_LOG='info',
                   VIBE_KANBAN_AGENT_PROCESS_HOST=str(ROOT / 'target/debug/agent-process-host'))
        with (instance / 'server.log').open('w+') as log:
            process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
            stream = None
            try:
                deadline = time.monotonic() + 90
                while time.monotonic() < deadline:
                    assert process.poll() is None, (instance / 'server.log').read_text()
                    port_file = instance / ('library-port' if mode == 'library' else 'ports/vibe-kanban.port')
                    if port_file.exists():
                        raw = port_file.read_text()
                        port = int(raw) if mode == 'library' else json.loads(raw)['main_port']
                        base = f'http://127.0.0.1:{port}'
                        try:
                            # Remains open until AFTER process exit/timeout; no browser teardown.
                            # Axum's nested root is /api/events; the trailing slash is a 404.
                            stream = urllib.request.urlopen(base + '/api/events', timeout=20)
                            assert stream.status == 200
                            assert stream.headers.get_content_type() == 'text/event-stream'
                            # Prove this is a live, valid SSE body, not just headers or a 404.
                            assert stream.readline(), 'SSE closed before shutdown'
                            break
                        except (OSError, urllib.error.URLError):
                            if stream is not None:
                                stream.close()
                                stream = None
                    time.sleep(.05)
                assert stream is not None, 'startup/SSE timeout'
                if close_sse:
                    stream.close()
                started = time.monotonic()
                signal_at = datetime.now(timezone.utc).isoformat()
                process.send_signal(signal.SIGTERM)
                hung = False
                try:
                    code = process.wait(timeout=12)
                    assert code == 0, (code, (instance / 'server.log').read_text())
                except subprocess.TimeoutExpired:
                    hung = True
                elapsed = time.monotonic() - started
                output = (instance / 'server.log').read_text()
                cleanup = 'Found 0 running processes to kill' in output
                print(json.dumps(dict(mode=mode, sse_status=200, sse_open_at_sigterm=not close_sse,
                                      sigterm_at=signal_at,
                                      hung=hung, exit_code=process.poll(), cleanup_reached=cleanup,
                                      elapsed_seconds=round(elapsed, 3),
                                      shutdown_log=[line for line in output.splitlines() if any(
                                          text in line for text in ['Shutdown signal received',
                                                                   'HTTP shutdown grace period elapsed',
                                                                   'Found 0 running processes to kill',
                                                                   'Library serve and cleanup completed'])])), flush=True)
                assert hung == expect_hang, output
                assert cleanup == (not expect_hang), output
                if close_sse:
                    assert elapsed < 2, 'closed SSE should not consume the grace period'
                    assert 'HTTP shutdown grace period elapsed' not in output, output
                if mode == 'library' and not expect_hang:
                    assert 'Library serve and cleanup completed' in output, output
            finally:
                # Kill/wait only our subprocess, even when reproducing the old hang.
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
                if stream is not None:
                    stream.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-test-binary', type=Path)
    parser.add_argument('--mode', choices=['main', 'library', 'both'], default='main')
    parser.add_argument('--expect-hang', action='store_true', help='baseline reproduction only')
    parser.add_argument('--close-sse-before-stop', action='store_true', help='fast graceful shutdown control')
    args = parser.parse_args()
    (ROOT / '.build').mkdir(exist_ok=True)
    if args.mode in ['library', 'both'] and not args.library_test_binary:
        parser.error('--library-test-binary is required for library/both mode')
    if args.expect_hang and args.close_sse_before_stop:
        parser.error('--expect-hang requires an open SSE')
    if args.mode in ['main', 'both']:
        run('main', [str(ROOT / 'target/debug/server')], args.expect_hang, args.close_sse_before_stop)
    if args.mode in ['library', 'both']:
        run('library', [str(args.library_test_binary.resolve()), '--ignored', '--exact',
                        'library_shutdown_fixture', '--nocapture'], args.expect_hang, args.close_sse_before_stop)
