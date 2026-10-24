"""No provider/network: wrapper -> native JSON-RPC fixture for Rust regression."""
import json
import os
from pathlib import Path
import subprocess
import sys

mode, record = sys.argv[1:3]
if mode == 'native':
    behavior = sys.argv[3]
    temporary = Path(record + '.tmp')
    temporary.write_text(json.dumps({'wrapper': os.getppid(), 'native': os.getpid()}))
    temporary.replace(record)
    for line in sys.stdin:
        request = json.loads(line)
        if behavior == 'normal' and 'id' in request:
            result = {'userAgent': 'codex_cli_rs/0.0.0', 'codexHome': str(Path(record).parent.resolve()), 'platformFamily': 'unix', 'platformOs': 'linux'} if request['method'] == 'initialize' else {'data': [], 'nextCursor': None}
            print(json.dumps({'id': request['id'], 'result': result}), flush=True)
else:
    native = subprocess.Popen([sys.executable, __file__, 'native', record, mode])
    if mode != 'wrapper-exit':
        native.wait()
    else:
        import time
        while not Path(record).exists():
            time.sleep(0.01)
