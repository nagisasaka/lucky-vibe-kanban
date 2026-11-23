"""Real developer-runtime split-switch acceptance in a disposable instance.
Synthetic frontend revisions deliberately identify fixtures, not source commits.
"""
import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import shutil
import secrets
import struct
import uuid
import subprocess
import sys
import time
import urllib.request
from unittest.mock import patch

import frontend_bundle as f
import frontend_switch
import start_binary as b
import start_support as s
from test_frontend_switch import artifact
from test_real_server import runtime_children, read_exact
from test_switch import SwitchTest


@contextmanager
def held_auxiliaries(fixture):
    release = Path(b.current()['release'])
    env = {'PATH': str(release) + ':/usr/bin:/bin', 'LVK_INSTANCE_DIR': str(fixture.instance),
           'MCP_HOST': '127.0.0.1', 'MCP_PORT': str(fixture.port)}
    processes = []
    try:
        host = subprocess.Popen([str(release / 'agent-process-host')], cwd=fixture.root,
            env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        processes.append(host)
        payload = json.dumps({'protocol_version': 2, 'run_attempt_id': str(uuid.uuid4()),
            'host_instance_id': str(uuid.uuid4()), 'auth_token': secrets.token_hex(32),
            'requested_endpoint': '127.0.0.1:0'}).encode()
        host.stdin.write(struct.pack('>I', len(payload)) + payload)
        host.stdin.flush()
        ready = json.loads(read_exact(host.stdout, struct.unpack('>I', read_exact(host.stdout, 4))[0]))
        assert ready['host_pid'] == host.pid
        mcp = subprocess.Popen([str(release / 'vibe-kanban-mcp')], cwd=fixture.root,
            env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        processes.append(mcp)
        mcp.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
            'params': {'protocolVersion': '2024-11-05', 'capabilities': {},
                       'clientInfo': {'name': 'split-test', 'version': '1'}}}).encode() + b'\n')
        mcp.stdin.flush()
        reply = b''
        while not reply.endswith(b'\n'):
            reply += read_exact(mcp.stdout, 1)
        assert 'result' in json.loads(reply)
        identities = [s.identity(process.pid) for process in processes]
        yield
        assert all(process.poll() is None for process in processes)
        assert [s.identity(process.pid) for process in processes] == identities
    finally:
        for process in reversed(processes):
            process.terminate()
            process.wait(timeout=10)
            process.stdin.close()
            process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--initial-bundle', type=Path)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--diagnostics', type=Path)
    parser.add_argument('--frontend-bundle', type=Path)
    parser.add_argument('--browser-output', type=Path)
    parser.add_argument('--browser-only', action='store_true')
    args = parser.parse_args()
    fixture = SwitchTest()
    fixture.setUp()
    try:
        fixture.old.terminate()
        fixture.old.wait(timeout=10)
        # Only fixture-owned initial DB is removed; real startup creates schema.
        for path in fixture.instance.joinpath('data').glob('db.v2.sqlite*'):
            path.unlink()
        token = fixture.instance / 'data/access-internal.token'
        token.write_text(secrets.token_urlsafe(32))
        token.chmod(0o600)
        (fixture.instance / 'data/config.json').unlink()
        release = fixture.instance / 'releases/real-a'
        shutil.copytree((args.initial_bundle or args.bundle).resolve(), release)
        initial_manifest = s.readjson(release / 'bundle.json')
        manifest = s.readjson(args.bundle.resolve() / 'bundle.json')
        a = args.frontend_bundle.resolve() if args.frontend_bundle else artifact(fixture.root, 'a')
        candidate = artifact(fixture.root, 'b')
        front = f.install(fixture.instance, a)
        old_chunk = next(name for name in front['files'] if name.startswith('assets/') and name.endswith('.js'))
        old_bytes = (a / old_chunk).read_bytes()
        selected = {'release': str(release), 'manifest': initial_manifest, 'frontend': front}
        s.atomic(fixture.instance / 'current.json', json.dumps(selected).encode())
        settings = {'port': fixture.port, 'environment': {
            'PATH': '/usr/local/bin:/usr/bin:/bin',
            'LVK_ACCESS_DEPLOYMENT': 'local', 'PREVIEW_PROXY_PORT': '0',
            'LVK_DEV_EXTERNAL_FRONTEND': '1',
            'VK_ALLOWED_ORIGINS': f'http://127.0.0.1:{fixture.port}',
        }}
        s.atomic(fixture.config, json.dumps(settings).encode())
        with (fixture.root / 'real.log').open('wb') as log:
            fixture.old = subprocess.Popen([sys.executable, str(fixture.instance / 'switch/launcher.py'),
                str(fixture.instance)], stdout=log, stderr=log)
        fixture.wait_port()
        b.configure(fixture.instance)
        b.health(fixture.old, selected)
        if args.browser_output:
            config = s.get('/api/info')['data']['config']
            config.update(onboarding_acknowledged=True, disclaimer_acknowledged=True, show_release_notes=False, analytics_enabled=False)
            request = urllib.request.Request(f'http://127.0.0.1:{fixture.port}/api/config',
                data=json.dumps(config).encode(), method='PUT',
                headers={'Content-Type': 'application/json', 'X-LVK-Internal-Token': token.read_text()})
            with urllib.request.urlopen(request) as response:
                assert json.load(response)['success']
            subprocess.run(['node', str(Path(__file__).with_name('verify-external-ui.mjs')),
                f'http://127.0.0.1:{fixture.port}', str(args.browser_output.resolve()), front['revision']], check=True)
        if args.browser_only:
            assert args.browser_output, '--browser-only requires --browser-output'
            return
        before = b.observation()
        backend = s.get('/api/info')['data']['build_info']['revision']
        # Record schema and data file hashes only after startup quiesces. Frontend
        # transaction itself is additionally covered by file-only fault tests.
        with held_auxiliaries(fixture):
            first = frontend_switch.switch(fixture.instance, candidate, 'Synthetic UI fixture, unchanged API')
            assert b.observation() == before
            info = s.get('/api/info')['data']['build_info']
            assert info['revision'] == backend and info['frontend_revision'] == 'b' * 40
            assert s.get_bytes(f.PREFIX + 'a' * 40 + '/' + old_chunk) == old_bytes
            assert b'bbbb' in s.get_bytes('/')
            rollback = frontend_switch.switch(fixture.instance, a, 'Fixture rollback')
            assert b.observation() == before
            assert s.get('/api/info')['data']['build_info']['frontend_revision'] == 'a' * 40
            invalid = fixture.root / 'invalid'
            shutil.copytree(candidate, invalid)
            (invalid / 'assets/chunk.js').chmod(0o600)
            (invalid / 'assets/chunk.js').write_text('corrupt')
            try:
                frontend_switch.switch(fixture.instance, invalid, 'Invalid fixture must fail')
                raise AssertionError('Corrupt dist accepted')
            except s.Refuse:
                pass
            with patch.object(f, 'health', side_effect=s.Refuse('Injected post-cutover HTTP failure')):
                try:
                    frontend_switch.switch(fixture.instance, candidate, 'Injected failure restores A')
                    raise AssertionError('Fault was not raised')
                except s.Refuse:
                    pass
            assert b.observation() == before
            assert s.get('/api/info')['data']['build_info']['frontend_revision'] == 'a' * 40
        runtime_children(fixture, args.source.resolve())
        # Existing backend switch engine must preserve selected dist exactly.
        fixture.builds[1] = {'bundle': str(args.bundle.resolve())}
        backend_start = time.monotonic()
        result = subprocess.run(fixture.cli(), capture_output=True, text=True, timeout=180)
        state = json.loads(result.stdout)
        state['measured_switch_seconds'] = time.monotonic() - backend_start
        assert result.returncode == 0 and state['status'] == 'succeeded', state
        after = b.current()
        assert after['frontend'] == front
        assert after['manifest']['files'] == manifest['files']
        if args.initial_bundle:
            assert initial_manifest['files']['server'] != manifest['files']['server']
            assert all(initial_manifest['files'][name] == manifest['files'][name]
                       for name in ['agent-process-host', 'vibe-kanban-mcp'])
        assert s.get('/api/info')['data']['build_info']['frontend_revision'] == 'a' * 40
        assert b.observation()['pid'] != before['pid']
        runtime_children(fixture, args.source.resolve())
        # A paired update selects the dist and backend together from one declared
        # target identity. The content is explicitly a synthetic UI fixture.
        paired_front = artifact(fixture.root, 'c', manifest['backend_revision'])
        paired = fixture.root / 'paired'
        shutil.copytree(args.bundle.resolve(), paired)
        shutil.copytree(paired_front, paired / 'frontend')
        paired_manifest = dict(manifest, frontend=f.inspect(paired_front))
        (paired / 'bundle.json').write_text(json.dumps(paired_manifest))
        fixture.builds[1] = {'bundle': str(paired)}
        paired_start = time.monotonic()
        result = subprocess.run(fixture.cli(), capture_output=True, text=True, timeout=180)
        paired_state = json.loads(result.stdout)
        paired_state['measured_switch_seconds'] = time.monotonic() - paired_start
        assert result.returncode == 0 and paired_state['status'] == 'succeeded', paired_state
        info = s.get('/api/info')['data']['build_info']
        assert info['frontend_revision'] == info['revision'] == manifest['backend_revision']
        assert s.get_bytes(f.PREFIX + 'a' * 40 + '/' + old_chunk) == old_bytes
        # Exercise the documented offline restore of the exact previous tuple.
        # These fixtures use the same backend/schema, so DB compatibility is
        # established here; this is not automatic production DB rollback.
        identity = b.observation()
        s.idle(identity['pid'], allow_children=True)
        s.stop(identity, b.current()['manifest']['files']['server'])
        s.stopped()
        b.verify_release(after)
        s.atomic(fixture.instance / 'current.json', json.dumps(after).encode())
        with (fixture.root / 'real.log').open('ab') as log:
            fixture.old = subprocess.Popen([sys.executable, str(fixture.instance / 'switch/launcher.py'),
                str(fixture.instance)], stdout=log, stderr=log)
        restored_info = b.health(fixture.old, after)
        assert restored_info['frontend_revision'] == front['revision']
        assert b.current() == after
        print(json.dumps({'frontend': first, 'rollback': rollback, 'backend': state, 'paired': paired_state, 'paired_rollback': restored_info,
                          'checks': 'server/host/MCP identities unchanged on frontend switch; old chunk; rollback; backend artifact updated with dist preserved; paired selection and offline tuple restore; real host and stdio MCP'}))
    finally:
        if args.diagnostics:
            args.diagnostics.mkdir(parents=True, exist_ok=True)
            if (fixture.root / 'real.log').exists():
                shutil.copy2(fixture.root / 'real.log', args.diagnostics / 'server.log')
        fixture.tearDown()
        # Immutable dist directories belong solely to this test.
        if fixture.root.exists():
            for path in fixture.root.rglob('*'):
                if path.is_dir():
                    path.chmod(0o700)
        fixture.doCleanups()


if __name__ == '__main__':
    main()
