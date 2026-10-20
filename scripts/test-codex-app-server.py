#!/usr/bin/env python3
"""Offline smoke of the real Codex binary. No account, shared DB, or model calls.

Run: python3 scripts/test-codex-app-server.py [codex command and arguments]
The mock Responses endpoint captures the native mode instructions actually sent
by App Server, rather than merely checking our turn/start JSON.
"""
from contextlib import ExitStack
import http.server
import json
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time


def mode_instructions(request):
    modes = [json.dumps(item) for item in request.get('input', [])
             if item.get('role') == 'developer' and any(
                 header in json.dumps(item) for header in
                 ['# Plan Mode (Conversational)', '# Collaboration Mode: Default'])]
    assert modes, 'native collaboration instructions missing'
    return modes[-1]


class Responses(http.server.BaseHTTPRequestHandler):
    requests = queue.Queue()
    release = threading.Event()
    release.set()

    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        self.requests.put(request)
        if not self.release.wait(timeout=20):
            raise TimeoutError("smoke response was not released")
        text = '<proposed_plan>\nOffline smoke plan.\n</proposed_plan>' if '# Plan Mode (Conversational)' in mode_instructions(request) else 'Smoke complete.'
        message = {'type': 'message', 'id': 'msg_smoke', 'role': 'assistant',
                   'status': 'completed', 'content': [{'type': 'output_text', 'text': text}]}
        events = [
            {'type': 'response.created', 'response': {'id': 'resp_smoke'}},
            {'type': 'response.output_item.added', 'output_index': 0,
             'item': {**message, 'status': 'in_progress', 'content': []}},
            {'type': 'response.content_part.added', 'item_id': 'msg_smoke', 'output_index': 0,
             'content_index': 0, 'part': {'type': 'output_text', 'text': ''}},
            {'type': 'response.output_text.delta', 'item_id': 'msg_smoke', 'output_index': 0,
             'content_index': 0, 'delta': text},
            {'type': 'response.output_item.done', 'output_index': 0, 'item': message},
            {'type': 'response.completed', 'response': {'id': 'resp_smoke', 'status': 'completed',
             'output': [message], 'usage': {'input_tokens': 10, 'output_tokens': 3, 'total_tokens': 13}}},
        ]
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for event in events:
            self.wfile.write(('data: ' + json.dumps(event) + '\n\n').encode())
        self.wfile.flush()


def main():
    command = sys.argv[1:] or ['codex']
    version = subprocess.check_output(command + ['--version'], text=True).strip()
    with tempfile.TemporaryDirectory(prefix='lvk-codex-smoke-') as root, ExitStack() as cleanup:
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Responses)
        server_thread = threading.Thread(target=server.serve_forever)
        server_thread.start()
        cleanup.callback(server_thread.join)
        cleanup.callback(server.server_close)
        cleanup.callback(server.shutdown)
        config = '\n'.join([
            'model = "gpt-5.4"', 'model_provider = "smoke"',
            '[model_providers.smoke]', 'name = "Offline smoke"',
            f'base_url = "http://127.0.0.1:{server.server_port}/v1"',
            'wire_api = "responses"', 'requires_openai_auth = false',
            'supports_websockets = false',
        ])
        with open(os.path.join(root, 'config.toml'), 'w') as file:
            file.write(config)
        events = queue.Queue()
        with open(os.path.join(root, 'stderr.log'), 'w+') as stderr:
            process = subprocess.Popen(command + ['app-server'], cwd=root,
                                       env={**os.environ, 'CODEX_HOME': root},
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=stderr, text=True)
            reader = threading.Thread(target=lambda: [events.put(json.loads(line)) for line in process.stdout])
            reader.start()
            seq = 0
            notifications = []

            def send(method, params, request_id=None):
                payload = {'method': method, 'params': params}
                if request_id is not None:
                    payload['id'] = request_id
                process.stdin.write(json.dumps(payload) + '\n')
                process.stdin.flush()

            def wait(predicate):
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    event = events.get(timeout=max(0.1, deadline - time.monotonic()))
                    if 'method' in event:
                        notifications.append(event)
                    if predicate(event):
                        return event
                raise TimeoutError('App Server response timed out')

            def rpc(method, params):
                nonlocal seq
                seq += 1
                send(method, params, seq)
                result = wait(lambda event: event.get('id') == seq)
                assert 'error' not in result, (method, result)
                return result['result']

            try:
                rpc('initialize', {'clientInfo': {'name': 'lvk_offline_smoke', 'version': '1'},
                                   'capabilities': {'experimentalApi': True}})
                send('initialized', {})
                started = rpc('thread/start', {'cwd': root, 'model': 'gpt-5.4',
                             'approvalPolicy': 'never', 'sandbox': 'read-only'})
                thread = started['thread']['id']
                for mode in ['plan', 'default']:
                    rpc('turn/start', {'threadId': thread, 'input': [{'type': 'text', 'text': 'Say smoke complete.'}],
                        'collaborationMode': {'mode': mode, 'settings': {'model': 'gpt-5.4',
                        'reasoning_effort': 'medium', 'developer_instructions': None}}})
                    completed = wait(lambda e: e.get('method') == 'turn/completed')
                    assert completed['params']['turn']['status'] == 'completed', completed
                    request = Responses.requests.get(timeout=5)
                    rendered = mode_instructions(request)
                    expected = '# Plan Mode (Conversational)' if mode == 'plan' else '# Collaboration Mode: Default'
                    assert expected in rendered, (mode, 'native mode instructions missing')
                    assert any(e.get('method') == 'item/completed' for e in notifications)
                    if mode == 'plan':
                        assert any(e.get('method') == 'item/completed' and
                                   e['params']['item']['type'] == 'plan' for e in notifications)
                    print(f'PASS {mode}: native instructions reached Responses; turn completed')
                resumed = rpc('thread/resume', {'threadId': thread})
                assert resumed['thread']['id'] == thread
                print('PASS resume: original thread retained')
                Responses.release.clear()
                active = rpc('turn/start', {'threadId': thread,
                    'input': [{'type': 'text', 'text': 'Wait for steering.'}]})
                Responses.requests.get(timeout=5)
                steered = rpc('turn/steer', {'threadId': thread,
                    'expectedTurnId': active['turn']['id'],
                    'input': [{'type': 'text', 'text': 'Keep the answer short.'}]})
                assert steered['turnId'] == active['turn']['id'], steered
                Responses.release.set()
                completed = wait(lambda e: e.get('method') == 'turn/completed')
                assert completed['params']['turn']['status'] == 'completed', completed
                print('PASS Steering: active turn accepted input and completed')
                goal = rpc('thread/goal/set', {'threadId': thread, 'objective': 'Smoke goal', 'status': 'paused'})
                assert goal['goal']['objective'] == 'Smoke goal', goal
                Responses.release.clear()
                while not Responses.requests.empty():
                    Responses.requests.get_nowait()
                active_goal = rpc('thread/goal/set', {'threadId': thread, 'status': 'active'})
                assert active_goal['goal']['status'] == 'active', active_goal
                Responses.requests.get(timeout=10)
                rpc('thread/goal/set', {'threadId': thread, 'status': 'paused'})
                Responses.release.set()
                completed = wait(lambda e: e.get('method') == 'turn/completed')
                assert completed['params']['turn']['status'] == 'completed', completed
                rpc('thread/goal/clear', {'threadId': thread})
                print('PASS Goal: activate, run, pause and clear')
                fixture = os.environ.get('LVK_CODEX_SMOKE_FIXTURE')
                if fixture:
                    # Temp paths are not meaningful fixture identity. No credentials
                    # are used; only protocol responses/notifications are retained.
                    data = json.dumps({'version': version, 'start': started, 'resume': resumed,
                        'goal': goal, 'notifications': [e for e in notifications if e['method'] in {
                            'item/completed', 'turn/completed', 'thread/tokenUsage/updated',
                            'thread/goal/updated', 'thread/goal/cleared'}]}, indent=2).replace(root, '/tmp/lvk-codex-smoke')
                    with open(fixture, 'w') as file:
                        file.write(data + '\n')
                print(f'PASS {version}')
            except BaseException:
                stderr.seek(0)
                print(stderr.read()[-3000:], file=sys.stderr)
                raise
            finally:
                Responses.release.set()
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
                reader.join(timeout=5)


if __name__ == '__main__':
    main()
