#!/usr/bin/env python3
"""Non-inference app-server fixture: models/config fail independently; resume retains old values unless sent."""
import json
import os
import sys
for line in sys.stdin:
    req = json.loads(line)
    if 'id' not in req:
        continue
    method, params = req['method'], req.get('params', {})
    with open(os.environ['LVK480_RECORD'], 'a') as log:
        log.write(json.dumps(req) + '\n')
    mode = os.environ.get('LVK480_CASE', '')
    if method == 'model/list' and mode == 'models-timeout':
        continue
    if (method in ['config/read','model/list'] and mode == 'both-fail') or (method == 'model/list' and mode == 'models-fail') or (method == 'config/read' and mode == 'config-fail'):
        print(json.dumps({'id': req['id'], 'error': {'code': -32000, 'message': 'fixture read failure'}}), flush=True)
        continue
    if method == 'initialize':
        result = {'userAgent': 'fixture/1', 'codexHome': os.getcwd(), 'platformFamily': 'unix', 'platformOs': 'linux'}
    elif method == 'config/read':
        result = {'config': {'model': 'astra', 'model_reasoning_effort': 'medium'} if mode != 'empty' else {}, 'origins': {}, 'layers': None}
    elif method == 'model/list':
        result = {'data': [{'id': 'astra', 'model': 'astra', 'displayName': 'Astra', 'description': 'fixture',
                            'hidden': False, 'isDefault': True, 'defaultReasoningEffort': 'medium',
                            'supportedReasoningEfforts': [{'reasoningEffort': 'medium', 'description': 'fixture'}]}], 'nextCursor': None}
    elif method == 'thread/resume':
        result = {'thread': {'id': params['threadId'], 'path': None},
                  'model': params.get('model') or 'previous-model',
                  'reasoningEffort': params.get('config', {}).get('model_reasoning_effort') or 'high'}
    else:
        raise AssertionError('Unexpected method: ' + method)
    print(json.dumps({'id': req['id'], 'result': result}), flush=True)
