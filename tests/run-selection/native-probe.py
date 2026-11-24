#!/usr/bin/env python3
"""Optional real-CLI, offline, non-inference evidence. No user turn is sent.
Uses disposable homes/cwd and only projects model/effort (never auth/settings).
Run: python3 tests/run-selection/native-probe.py
"""
import json
import os
from pathlib import Path
import queue
import shutil
import signal
import subprocess
import tempfile
import threading


class Peer:
    def __init__(self, command, directory, env, claude=False):
        self.claude = claude
        self.seq = 0
        self.events = queue.Queue()
        self.child = subprocess.Popen(command, cwd=directory, env=env, stdin=subprocess.PIPE,
                                      stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                      text=True, start_new_session=True)
        self.reader = threading.Thread(target=self.read)
        self.reader.start()

    def read(self):
        for line in self.child.stdout:
            self.events.put(json.loads(line))

    def rpc(self, name, params=None, error=False):
        self.seq += 1
        if self.claude:
            value = {'type': 'control_request', 'request_id': str(self.seq),
                     'request': {'subtype': name, **(params or {})}}
        else:
            value = {'id': self.seq, 'method': name, 'params': params or {}}
        self.child.stdin.write(json.dumps(value) + '\n')
        self.child.stdin.flush()
        while True:
            v = self.events.get(timeout=15)
            if self.claude and v.get('type') == 'control_response':
                assert v['response']['subtype'] == 'success', v
                return v['response'].get('response', {})
            if not self.claude and v.get('id') == self.seq:
                if error:
                    return v
                assert 'error' not in v, v
                return v['result']
            assert 'id' not in v, 'Unexpected server request; never approve'

    def close(self):
        os.killpg(self.child.pid, signal.SIGTERM)
        try:
            self.child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(self.child.pid, signal.SIGKILL)
            self.child.wait(timeout=5)
        self.reader.join(timeout=5)
        assert not self.reader.is_alive()


def claude_probe(binary):
    for configured in [False, True]:
        with tempfile.TemporaryDirectory(prefix='lvk480-claude-') as d:
            content = {'model': 'sonnet', 'effortLevel': 'medium'} if configured else {}
            settings = Path(d, 'settings.json')
            settings.write_text(json.dumps(content))
            before = settings.read_bytes()
            env = {'PATH': os.environ['PATH'], 'HOME': d, 'CLAUDE_CONFIG_DIR': d,
                   'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1'}
            command = [binary, '-p', '--input-format=stream-json', '--output-format=stream-json',
                       '--verbose', '--no-session-persistence', '--strict-mcp-config', '--mcp-config',
                       '{"mcpServers":{}}', '--tools', '', '--settings', '{"disableAllHooks":true}']
            for override in [None, 'opus']:
                peer = Peer(command + (['--model', override] if override else []), d, env, True)
                try:
                    init = peer.rpc('initialize')
                    result = peer.rpc('get_settings')
                    applied = result.get('applied', {})
                    assert isinstance(applied.get('model'), str), 'No verified model from this CLI'
                    assert isinstance(applied.get('effort'), str), 'No verified effort from this CLI'
                    if configured and override is None:
                        assert applied['effort'] == 'medium'
                    print(json.dumps({'provider': 'claude', 'configured': configured,
                                      'model_override': override, 'model': applied['model'],
                                      'effort': applied['effort'], 'capabilities': len(init.get('models', []))}))
                finally:
                    peer.close()
            assert settings.read_bytes() == before


def codex_probe(binary):
    with tempfile.TemporaryDirectory(prefix='lvk480-codex-') as d:
        config = Path(d, 'config.toml')
        config.write_text('')
        env = {'PATH': os.environ['PATH'], 'HOME': d, 'CODEX_HOME': d}
        peer = Peer([binary, 'app-server'], d, env)
        try:
            peer.rpc('initialize', {'clientInfo': {'name': 'lvk480_fixture', 'version': '1'},
                                    'capabilities': {'experimentalApi': True}})
            settings = peer.rpc('config/read', {'includeLayers': False, 'cwd': d})['config']
            models = peer.rpc('model/list')['data']
            default = next(m for m in models if m['isDefault'] and not m.get('hidden'))
            model, effort = default['id'], default['defaultReasoningEffort']
            assert settings.get('model') is None and settings.get('model_reasoning_effort') is None
            params = {'cwd': d, 'model': model, 'config': {'model_reasoning_effort': effort},
                      'sandbox': 'read-only'}
            initial = peer.rpc('thread/start', params)
            assert initial['model'] == model and initial['reasoningEffort'] == effort
            # No inference is needed to exercise the resume request. Some CLIs
            # refuse resuming a thread with no persisted user turn; report that.
            resumed = peer.rpc('thread/resume', {**params, 'threadId': initial['thread']['id']}, error=True)
            if 'result' in resumed:
                assert resumed['result']['model'] == model
                assert resumed['result']['reasoningEffort'] == effort
            print(json.dumps({'provider':'codex','model':model,'effort':effort,
                              'resume': 'confirmed' if 'result' in resumed else 'unverified: empty thread rejected'}))
            assert config.read_text() == ''
        finally:
            peer.close()


if __name__ == '__main__':
    for name, run in [('claude', claude_probe), ('codex', codex_probe)]:
        binary = shutil.which(name)
        if not binary:
            print(name + ': unavailable')
            continue
        print(subprocess.check_output([binary, '--version'], text=True).strip())
        run(binary)
