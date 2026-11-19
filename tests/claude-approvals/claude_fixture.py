#!/usr/bin/env python3
"""Official Claude control framing with no model, tools or credential reads.

This process only describes operations. It records approval responses and
asserts correlation and unchanged input; it never executes the described tool.
"""
import json
import os
from pathlib import Path
import sys
import uuid

if '--version' in sys.argv:
    print('2.1.308 (Claude Code approval fixture)')
    raise SystemExit(0)
if '--help' in sys.argv:
    print('--effort --permission-mode --permission-prompt-tool')
    raise SystemExit(0)

assert '--dangerously-skip-permissions' not in sys.argv
assert '--permission-mode=default' in sys.argv

trace = open(Path(os.environ['LVK_APPROVAL_FIXTURE_TRACE']) / f'{os.getpid()}.jsonl',
             'a', buffering=1)
session = str(uuid.uuid4())
pending = {}


def record(direction, value):
    # Runtime initialization/user frames can contain internal MCP credentials.
    if direction == 'client_to_provider' and value.get('type') != 'control_response':
        value = {'type': value.get('type'), 'subtype': value.get('request', {}).get('subtype')}
    trace.write(json.dumps({'direction': direction, 'value': value}) + '\n')


def emit(value):
    record('provider_to_client', value)
    print(json.dumps(value), flush=True)


def finish():
    emit({'type': 'result', 'subtype': 'success', 'is_error': False,
          'result': 'Claude approval fixture complete', 'session_id': session})


def request(kind):
    tool = 'Bash' if kind == 'command' else 'Write'
    inputs = ({'command': 'echo CLAUDE_APPROVAL', 'description': 'Isolated approval fixture'}
              if kind == 'command' else
              {'file_path': '/tmp/claude-approval-fixture.txt', 'content': 'fixture content'})
    native_id = f'claude-native-{kind}'
    call = f'claude-tool-{kind}'
    # File requests deliberately omit the assistant snapshot: details must also
    # be available from the canonical snapshot emitted by the approval handler.
    if kind == 'command':
        emit({'type': 'assistant', 'session_id': session, 'message': {
            'id': f'msg-{kind}', 'role': 'assistant', 'content': [
                {'type': 'tool_use', 'id': call, 'name': tool, 'input': inputs}]}})
    pending[native_id] = inputs
    emit({'type': 'control_request', 'request_id': native_id, 'request': {
        'subtype': 'can_use_tool', 'tool_name': tool, 'input': inputs,
        'tool_use_id': call, 'decision_reason': 'Isolated approval fixture'}})


for line in sys.stdin:
    value = json.loads(line)
    record('client_to_provider', value)
    if value.get('type') == 'control_request':
        subtype = value['request']['subtype']
        if subtype == 'interrupt':
            finish()
            break
        assert subtype in ('initialize', 'set_permission_mode'), subtype
        if subtype == 'set_permission_mode':
            assert value['request']['mode'] == 'default'
        emit({'type': 'control_response', 'response': {
            'subtype': 'success', 'request_id': value['request_id'], 'response': {}}})
    elif value.get('type') == 'user':
        emit({'type': 'system', 'subtype': 'init', 'session_id': session})
        prompt = json.dumps(value)
        if 'FIXTURE_PARALLEL' in prompt:
            request('command')
            request('file')
        else:
            request('file' if 'FIXTURE_FILE' in prompt else 'command')
    elif value.get('type') == 'control_response':
        response = value['response']
        original = pending.pop(response['request_id'])
        assert response['subtype'] == 'success', response
        decision = response['response']
        assert decision['behavior'] in ('allow', 'deny')
        assert 'updatedPermissions' not in decision
        if decision['behavior'] == 'allow':
            assert decision['updatedInput'] == original
        if not pending:
            finish()
            break
    else:
        raise AssertionError(f'Unexpected fixture frame type: {value.get("type")}')
