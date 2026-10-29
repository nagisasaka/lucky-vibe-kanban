#!/usr/bin/env python3
"""Offline provider only; the browser, API, DB and host use real source code."""
import json
import os
from pathlib import Path
import sys
import uuid

if '--version' in sys.argv:
    print('codex-cli 0.163.0 (REVIEW fixture)')
    raise SystemExit(0)
root = Path(__file__).resolve().parents[2]
start = json.loads((root / 'crates/executors/fixtures/codex-app-server-0.163.0.json').read_text())['start']
thread = start['thread']['id']
turn = str(uuid.uuid4())
trace_dir = os.environ.get('LVK_REVIEW_FIXTURE_TRACE')
trace = open(Path(trace_dir) / f'{os.getpid()}.jsonl', 'a', buffering=1) if trace_dir else None
review = False


def send(value):
    print(json.dumps(value), flush=True)


def notify(method, params):
    send({'method': method, 'params': {'threadId': thread, **params}})


for line in sys.stdin:
    value = json.loads(line)
    method = value.get('method')
    if not method or 'id' not in value:
        continue
    params = value.get('params', {})
    if trace:
        # Never record inherited credentials, full prompts or developer context.
        trace.write(json.dumps({'method': method, 'review': 'LVK REVIEW mode:' in params.get('developerInstructions', ''),
                                'approval': params.get('approvalPolicy'), 'sandbox': params.get('sandbox'),
                                'collaboration_mode': params.get('collaborationMode', {}).get('mode')}) + '\n')
    if method == 'initialize':
        result = {'userAgent': 'codex/0.163.0', 'codexHome': '/tmp/review-fixture', 'platformFamily': 'unix', 'platformOs': 'linux'}
    elif method == 'config/read':
        result = {'config': {}, 'origins': {}, 'layers': []}
    elif method == 'account/read':
        result = {'account': None, 'requiresOpenaiAuth': False}
    elif method in ('model/list', 'skills/list'):
        result = {'data': [], 'nextCursor': None}
    elif method == 'account/rateLimits/read':
        result = {'rateLimits': {'primary': None, 'secondary': None, 'credits': None, 'planType': None}, 'rateLimitsByLimitId': {}}
    elif method in ('thread/start', 'thread/resume'):
        thread = str(uuid.uuid4()) if method == 'thread/start' else params['threadId']
        start['thread']['id'] = thread
        review = 'LVK REVIEW mode:' in params.get('developerInstructions', '')
        result = start
    elif method == 'thread/inject_items':
        result = {}
    elif method == 'thread/read':
        result = {'thread': start['thread']}
    elif method == 'thread/settings/update':
        result = start
    elif method == 'turn/start':
        turn = str(uuid.uuid4())
        result = {'turn': {'id': turn, 'items': [], 'status': 'inProgress', 'error': None}}
    else:
        raise RuntimeError(f'Unsupported REVIEW fixture method: {method}')
    send({'id': value['id'], 'result': result})
    if method == 'thread/start':
        notify('thread/started', {'thread': start['thread']})
    if method == 'turn/start':
        notify('turn/started', {'turn': result['turn']})
        text = 'REVIEW fixture findings: report and stop.' if review else 'CODE fixture complete.'
        notify('item/completed', {'turnId': turn, 'item': {'type': 'agentMessage', 'id': 'final', 'text': text}})
        notify('turn/completed', {'turn': {'id': turn, 'items': [], 'status': 'completed', 'error': None}})
