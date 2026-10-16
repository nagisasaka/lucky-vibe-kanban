"""Authenticate the operator CLI only to its selected local server."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class ControlAuthenticationTests(unittest.TestCase):
    def test_internal_capability_is_loopback_only_and_never_printed(self):
        observed = []
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                observed.append(dict(self.headers))
                body = json.dumps({'success': True, 'data': []}).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            def log_message(self, *args):
                pass
        with tempfile.TemporaryDirectory() as directory:
            token = Path(directory) / 'access-internal.token'
            token.write_text('fixture-internal-capability')
            token.chmod(0o600)
            env = dict(os.environ)
            env.pop('LVK_HTTP_AUTH', None)
            server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            script = str(Path(__file__).with_name('control.py'))
            try:
                command = [sys.executable, script, '--url', f'http://127.0.0.1:{server.server_port}', '--internal-token-file', str(token), 'runners']
                result = subprocess.run(command, capture_output=True, text=True, env=env, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(observed[0]['X-Lvk-Internal-Token'], token.read_text())
                self.assertNotIn(token.read_text(), result.stdout + result.stderr)
                command[3] = 'https://example.invalid'
                denied = subprocess.run(command, capture_output=True, text=True, env=env, timeout=10)
                self.assertNotEqual(denied.returncode, 0)
                self.assertIn('same-host loopback', denied.stderr)
                self.assertEqual(len(observed), 1)
            finally:
                server.shutdown()
                server.server_close()
                thread.join()
