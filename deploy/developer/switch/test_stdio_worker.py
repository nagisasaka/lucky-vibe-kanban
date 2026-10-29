"""Worker MCP bootstrap against an isolated instance, without a model or merge."""
import json
import os
from pathlib import Path
import select
import subprocess
import time
import unittest

import test_switch


class WorkerBootstrapTest(unittest.TestCase):
    def test_explicit_runtime_binding_survives_codex_environment_filter(self):
        binary = os.environ.get("LVK_TEST_MCP_BINARY")
        if not binary:
            self.skipTest("set LVK_TEST_MCP_BINARY to a built stdio binary")
        binary = str(Path(binary).resolve())
        fixture = test_switch.SwitchTest()
        fixture.setUp()
        self.addCleanup(fixture.tearDown)
        # Codex inherits PATH/TMPDIR but drops LVK_INSTANCE_DIR and backend port.
        # Keep the missing fallback port file isolated from any live instance.
        env = {"PATH": "/usr/bin:/bin", "TMPDIR": str(fixture.root), "RUST_LOG": "error"}
        initialize = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "worker-bootstrap-fixture", "version": "1"}}}
        failed = subprocess.run([binary], input=json.dumps(initialize) + "\n",
                                env=env, cwd=fixture.root, capture_output=True,
                                text=True, timeout=10)
        self.assertNotEqual(failed.returncode, 0)
        self.assertNotIn('"id":1', failed.stdout)
        self.assertIn("port", failed.stderr.lower())

        # These bindings are explicitly placed in worker ThreadStartParams.env;
        # the separate Rust contract test checks normal/resume configuration.
        env.update(LVK_INSTANCE_DIR=str(fixture.instance), MCP_HOST="127.0.0.1",
                   MCP_PORT=str(fixture.port), VK_AGENT_RUN_ID="fixture-worker")
        proc = subprocess.Popen([binary], env=env, cwd=fixture.root,
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, bufsize=0)
        try:
            messages = [initialize, {"jsonrpc": "2.0", "method": "notifications/initialized"},
                        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}]
            proc.stdin.write(("".join(json.dumps(m) + "\n" for m in messages)).encode())
            proc.stdin.flush()
            pending = b""
            replies = {}
            deadline = time.monotonic() + 10
            while 2 not in replies:
                remaining = deadline - time.monotonic()
                self.assertGreater(remaining, 0, "initialize/tools-list timeout")
                self.assertTrue(select.select([proc.stdout], [], [], remaining)[0])
                chunk = os.read(proc.stdout.fileno(), 65536)
                self.assertTrue(chunk, "MCP closed before replying")
                pending += chunk
                while b"\n" in pending:
                    line, pending = pending.split(b"\n", 1)
                    value = json.loads(line)
                    if "id" in value:
                        replies[value["id"]] = value
            self.assertIn("result", replies[1])
            names = {tool["name"] for tool in replies[2]["result"]["tools"]}
            self.assertIn("integration_action", names)
        finally:
            proc.stdin.close()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.terminate()
                proc.wait(timeout=5)
            proc.stdout.close()


if __name__ == "__main__":
    unittest.main()
