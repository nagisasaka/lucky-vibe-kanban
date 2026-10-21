#!/usr/bin/env python3
"""Deterministic app-server fixture: no model, tool execution or credentials.

Real LVK launch/client/host/API/UI code runs around this process. Commands and
file changes are protocol descriptions only; decisions are recorded verbatim.
"""
import json
import os
from pathlib import Path
import sys
import time
import uuid

if '--version' in sys.argv:
    print('codex-cli 0.144.1 (LOCAL-38 fixture)')
    raise SystemExit(0)

root = Path(__file__).resolve().parents[2]
start = json.loads((root / 'crates/executors/fixtures/codex-app-server-0.163.0.json').read_text())['start']
thread = start['thread']['id']
turn = 'approval-fixture-turn'
trace_dir = os.environ.get('LVK_APPROVAL_FIXTURE_TRACE')
trace = open(Path(trace_dir) / f'{os.getpid()}.jsonl', 'a', buffering=1) if trace_dir else None
pending = {}
scenario = 'command'


def record(direction, value):
    if trace:
        if direction == 'client_to_provider' and value.get('method') not in ('fixture/go', 'turn/interrupt') and 'decision' not in value.get('result', {}):
            # Launch/turn params may carry internal MCP credentials or prompts.
            # Only approval requests/responses need full payload evidence.
            value = {key: value[key] for key in ('id', 'method') if key in value}
        trace.write(json.dumps({'direction': direction, 'value': value}) + '\n')


def send(value):
    record('provider_to_client', value)
    print(json.dumps(value), flush=True)


def notification(method, params):
    send({'method': method, 'params': {'threadId': thread, **params}})


def request_approval(kind):
    item_id = f'{kind}-item'
    if kind == 'command':
        item = {'type': 'commandExecution', 'id': item_id, 'command': 'echo LOCAL38_APPROVAL',
                'cwd': '/tmp', 'processId': None, 'status': 'inProgress', 'commandActions': [],
                'aggregatedOutput': None, 'exitCode': None, 'durationMs': None}
        method = 'item/commandExecution/requestApproval'
    else:
        item = {'type': 'fileChange', 'id': item_id, 'status': 'inProgress',
                'changes': [{'path': '/tmp/local38-fixture.txt', 'kind': {'type': 'add'}, 'diff': '+fixture'}]}
        method = 'item/fileChange/requestApproval'
    notification('item/started', {'turnId': turn, 'item': item})
    native_id = 701 if kind == 'command' else 'file-native-702'
    pending[str(native_id)] = kind
    send({'id': native_id, 'method': method, 'params': {
        'threadId': thread, 'turnId': turn, 'itemId': item_id, 'startedAtMs': int(time.time()*1000),
        'reason': 'LOCAL-38 isolated approval fixture', 'command': 'echo LOCAL38_APPROVAL',
        'cwd': '/tmp', 'commandActions': [],
    }})


def finish(status='completed'):
    notification('item/completed', {'turnId': turn, 'item': {'type': 'agentMessage', 'id': 'final', 'text': 'LOCAL38 fixture complete'}})
    notification('turn/completed', {'turn': {'id': turn, 'items': [], 'status': status, 'error': None}})


for line in sys.stdin:
    value = json.loads(line)
    record('client_to_provider', value)
    method = value.get('method')
    if method == 'initialize':
        result = {'userAgent': 'codex/0.144.1', 'codexHome': '/tmp/local38-fixture-codex', 'platformFamily': 'unix', 'platformOs': 'linux'}
    elif method == 'config/read':
        result = {'config': {}, 'origins': {}, 'layers': []}
    elif method == 'account/read':
        result = {'account': None, 'requiresOpenaiAuth': False}
    elif method in ('model/list', 'skills/list'):
        result = {'data': [], 'nextCursor': None}
    elif method == 'account/rateLimits/read':
        result = {'rateLimits': {'primary': None, 'secondary': None, 'credits': None, 'planType': None}, 'rateLimitsByLimitId': {}}
    elif method in ('thread/start', 'thread/resume'):
        thread = str(uuid.uuid4()) if method == 'thread/start' else value['params']['threadId']
        start['thread']['id'] = thread
        result = start
    elif method == 'turn/start':
        result = {'turn': {'id': turn, 'items': [], 'status': 'inProgress', 'error': None}}
    elif method == 'turn/interrupt':
        send({'id': value['id'], 'result': {}})
        finish('interrupted')
        continue
    elif method == 'fixture/go':
        scenario = value.get('scenario', 'command')
        notification('turn/started', {'turn': {'id': turn, 'items': [], 'status': 'inProgress', 'error': None}})
        request_approval(scenario)
        continue
    elif 'result' in value and str(value.get('id')) in pending:
        kind = pending.pop(str(value['id']))
        if scenario == 'both' and kind == 'command':
            request_approval('file')
        elif not pending:
            finish()
        continue
    elif 'id' not in value:
        continue
    else:
        raise RuntimeError(f'Unsupported fixture method: {method}')
    send({'id': value['id'], 'result': result})
    if method == 'thread/start':
        notification('thread/started', {'thread': start['thread']})
    if method == 'turn/start':
        prompt = json.dumps(value.get('params', {}).get('input', []))
        scenario = 'parallel' if 'FIXTURE_PARALLEL' in prompt else ('file' if 'FIXTURE_FILE' in prompt else ('both' if 'FIXTURE_BOTH' in prompt else 'command'))
        notification('turn/started', {'turn': result['turn']})
        if 'User feedback:' in prompt:
            finish()
        elif scenario == 'parallel':
            request_approval('command')
            request_approval('file')
        else:
            request_approval('command' if scenario == 'both' else scenario)
