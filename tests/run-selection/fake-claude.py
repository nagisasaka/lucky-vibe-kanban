#!/usr/bin/env python3
"""Official control-wire fixture. Never accepts a user turn or writes settings."""
import json
import os
import sys
args = sys.argv[1:]
with open(os.environ['LVK480_RECORD'], 'a') as log:
    log.write(json.dumps({'args': args}) + '\n')
mode = os.environ.get('LVK480_CASE', '')
model = args[args.index('--model') + 1] if '--model' in args else 'astra'
effort = args[args.index('--effort') + 1] if '--effort' in args else ('low' if model == 'beta' else 'medium')
for line in sys.stdin:
    req = json.loads(line)
    assert req['type'] == 'control_request'
    name = req['request']['subtype']
    assert name in ['initialize', 'get_settings'], name
    error = (name == 'initialize' and mode == 'models-fail') or (name == 'get_settings' and mode == 'settings-fail')
    if name == 'initialize':
        result = {'models': [{'value': m, 'resolvedModel': m, 'displayName': m.title(),
                              'supportsEffort': m != 'unsupported',
                              'supportedEffortLevels': [] if m == 'unsupported' else ['low', 'medium', 'high']}
                             for m in ['astra', 'beta', 'unsupported']]}
    else:
        result = {'effective': {}, 'applied': {'model': model, 'effort': effort}}
        if mode == 'model-missing':
            result['applied'].pop('model')
        if mode == 'effort-missing':
            result['applied'].pop('effort')
    response = {'subtype': 'error' if error else 'success', 'request_id': req['request_id']}
    if error:
        response['error'] = 'fixture read failure'
    else:
        response['response'] = result
    print(json.dumps({'type': 'control_response', 'response': response}), flush=True)
