#!/usr/bin/env python3
"""Real Codex + offline Responses/MCP fixtures; no account or live LVK access.

Tests native Auto decisions independently from LVK's stdio transport test.
The approval-required tool only writes a marker in its disposable fixture.
"""
import http.server
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import time


def mcp():
    for line in sys.stdin:
        request = json.loads(line)
        if "id" not in request:
            continue
        method = request["method"]
        if method == "initialize":
            result = {"protocolVersion": request["params"]["protocolVersion"], "capabilities": {"tools": {}}, "serverInfo": {"name": "approval-fixture", "version": "1"}}
        elif method == "tools/list":
            result = {"tools": [{"name": name, "description": "Offline approval fixture", "inputSchema": {"type": "object", "properties": {}}, "annotations": {"readOnlyHint": readonly}} for name, readonly in [("get_context", True), ("write_fixture", False)]]}
        elif method == "tools/call":
            name = request["params"]["name"]
            with open("calls.jsonl", "a") as output:
                output.write(json.dumps({"tool": name}) + "\n")
            result = {"content": [{"type": "text", "text": "fixture success"}]}
        else:
            result = {}
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)


class Responses(http.server.BaseHTTPRequestHandler):
    tool = "get_context"
    issued = False

    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if not Responses.issued:
            namespace = next(t for t in request["tools"] if t.get("type") == "namespace" and any(f.get("name") == Responses.tool for f in t.get("tools", [])))
            name = Responses.tool
            Responses.issued = True
            item = {"type": "function_call", "id": "fc_fixture", "call_id": "call_fixture", "name": name, "namespace": namespace["name"], "arguments": "{}", "status": "completed"}
        else:
            item = {"type": "message", "id": "msg_fixture", "role": "assistant", "status": "completed", "content": [{"type": "output_text", "text": "Done."}]}
        events = [
            {"type": "response.created", "response": {"id": "resp_fixture"}},
            {"type": "response.output_item.added", "output_index": 0, "item": item},
            {"type": "response.output_item.done", "output_index": 0, "item": item},
            {"type": "response.completed", "response": {"id": "resp_fixture", "status": "completed", "output": [item], "usage": {"input_tokens": 10, "output_tokens": 3, "total_tokens": 13}}},
        ]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for event in events:
            self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
        self.wfile.flush()


def probe(command, policy, tool):
    Responses.tool, Responses.issued = tool, False
    with tempfile.TemporaryDirectory(prefix="lvk-mcp-approval-") as directory:
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Responses)
        http_thread = threading.Thread(target=server.serve_forever)
        http_thread.start()
        root = Path(directory)
        (root / "config.toml").write_text("\n".join([
            'model = "gpt-5.4"', 'model_provider = "fixture"',
            f'approval_policy = "{policy}"',
            '[model_providers.fixture]', 'name = "Offline fixture"',
            f'base_url = "http://127.0.0.1:{server.server_port}/v1"',
            'wire_api = "responses"', 'requires_openai_auth = false', 'supports_websockets = false',
            '[mcp_servers.vibe_kanban]', f'command = {json.dumps(sys.executable)}',
            f'args = {json.dumps([str(Path(__file__).resolve()), "--mcp"])}',
            f'cwd = {json.dumps(directory)}',
            '[mcp_servers.vibe_kanban.tools.get_context]', 'approval_mode = "auto"',
            '[mcp_servers.vibe_kanban.tools.write_fixture]', 'approval_mode = "auto"',
        ]))
        events = queue.Queue()
        with tempfile.TemporaryFile(mode="w+") as errors:
            process = subprocess.Popen(command + ["app-server"], cwd=directory, env={**os.environ, "CODEX_HOME": directory}, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors, text=True)
            reader = threading.Thread(target=lambda: [events.put(json.loads(line)) for line in process.stdout])
            reader.start()
            sequence = 0

            def send(payload):
                process.stdin.write(json.dumps(payload) + "\n")
                process.stdin.flush()

            def rpc(method, params):
                nonlocal sequence
                sequence += 1
                send({"id": sequence, "method": method, "params": params})
                while True:
                    event = events.get(timeout=20)
                    if event.get("id") == sequence:
                        assert "error" not in event, event
                        return event["result"]

            approvals = 0
            try:
                rpc("initialize", {"clientInfo": {"name": "lvk_mcp_fixture", "version": "1"}, "capabilities": {"experimentalApi": True}})
                send({"method": "initialized", "params": {}})
                started = rpc("thread/start", {"cwd": directory, "sandbox": "workspace-write"})
                assert started["approvalPolicy"] == policy
                thread = started["thread"]["id"]
                rpc("turn/start", {"threadId": thread, "input": [{"type": "text", "text": "Call the fixture tool."}]})
                deadline = time.monotonic() + 30
                while True:
                    event = events.get(timeout=max(0.1, deadline - time.monotonic()))
                    if "id" in event and "method" in event:
                        assert event["method"] == "mcpServer/elicitation/request", event["method"]
                        approvals += 1
                        # Deny explicitly; never weaken the native decision.
                        send({"id": event["id"], "result": {"action": "decline", "content": None, "_meta": None}})
                    if event.get("method") == "turn/completed":
                        assert event["params"]["turn"]["status"] == "completed", event
                        break
                calls = (root / "calls.jsonl").read_text() if (root / "calls.jsonl").exists() else ""
                assert approvals == int(tool == "write_fixture" and policy == "on-request"), approvals
                assert bool(calls) == (tool == "get_context"), calls
                print(json.dumps({"policy": policy, "tool": tool, "approvals": approvals, "executed": bool(calls)}))
            finally:
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
                reader.join(timeout=5)
                server.shutdown()
                server.server_close()
                http_thread.join(timeout=5)
                assert not reader.is_alive() and not http_thread.is_alive()


if __name__ == "__main__":
    if sys.argv[1:] == ["--mcp"]:
        mcp()
    else:
        command = sys.argv[1:] or ["codex"]
        version = subprocess.check_output(command + ["--version"], text=True).strip()
        assert version == "codex-cli 0.163.0", version
        print(version, flush=True)
        for policy in ["on-request", "never"]:
            for tool in ["get_context", "write_fixture"]:
                probe(command, policy, tool)
