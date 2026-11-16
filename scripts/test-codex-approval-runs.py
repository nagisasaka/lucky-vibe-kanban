#!/usr/bin/env python3
"""Isolated real LVK normal/Auto-merge admission + native Codex, offline model.

Requires checkout-local server, agent-process-host and vibe-kanban-mcp builds.
Never requests integration_action/publication, merges, pushes or restarts.
All instance/config/Git/wire data is disposable; only concise results print.
"""
import http.server
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request
import urllib.error
import uuid

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("offline_mcp", ROOT / "scripts/test-codex-mcp-readonly.py")
offline = importlib.util.module_from_spec(spec)
spec.loader.exec_module(offline)


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def poll(read, accept, seconds=90):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = read()
        if accept(value):
            return value
        time.sleep(0.2)
    raise TimeoutError(str(value))


def main():
    version = subprocess.check_output(["codex", "--version"], text=True).strip()
    assert version == "codex-cli 0.163.0", version
    print(version, flush=True)
    for name in ["server", "agent-process-host", "vibe-kanban-mcp"]:
        path = ROOT / "target/debug" / name
        assert path.exists(), f"Build {name} first"
        with path.open("rb") as file:
            print(json.dumps({"artifact": name, "sha256": hashlib.file_digest(file, "sha256").hexdigest()}), flush=True)
    with tempfile.TemporaryDirectory(prefix="lvk-approval-runs-") as directory:
        root = Path(directory)
        instance, home, repo = root / "instance", root / "codex", root / "repo"
        for path in [instance / "data", home, repo]:
            path.mkdir(parents=True)
        backend, proxy = port(), port()
        model = http.server.ThreadingHTTPServer(("127.0.0.1", 0), offline.Responses)
        model_thread = threading.Thread(target=model.serve_forever)
        model_thread.start()
        wire = root / "wire.jsonl"
        base_config = "\n".join([
            'model = "gpt-5.4"', 'model_provider = "fixture"',
            '[model_providers.fixture]', 'name = "Offline fixture"',
            f'base_url = "http://127.0.0.1:{model.server_port}/v1"',
            'wire_api = "responses"', 'requires_openai_auth = false', 'supports_websockets = false',
            '[mcp_servers.vibe_kanban]',
            f'command = {json.dumps(str(ROOT / "target/debug/vibe-kanban-mcp"))}',
            '[mcp_servers.vibe_kanban.env]', f'MCP_PORT = "{backend}"', 'MCP_HOST = "127.0.0.1"',
            f'LVK_INSTANCE_DIR = {json.dumps(str(instance))}',
            '[mcp_servers.vibe_kanban.tools.get_context]', 'approval_mode = "auto"',
        ])
        profile = {"sandbox": "workspace-write", "base_command_override": f"python3 {ROOT}/scripts/lib/codex-audit-proxy.py", "env": {"CODEX_HOME": str(home), "LVK_FIXTURE_WIRE": str(wire), "LVK_FIXTURE_APPROVAL_INPUT": str(root / "input.jsonl"), "LVK_FIXTURE_CODEX": "/usr/local/bin/codex"}}
        (instance / "data/profiles.json").write_text(json.dumps({"executors": {"CODEX": {"DEFAULT": {"CODEX": profile}, "AUTO": {"CODEX": {**profile, "sandbox": "auto"}}, "EXPLICIT": {"CODEX": {**profile, "ask_for_approval": "never"}}}}}))
        env = {key: value for key, value in os.environ.items() if not key.startswith(("LVK_", "VK_", "VIBE_")) and key not in ["DATABASE_URL", "BACKEND_PORT", "FRONTEND_PORT", "PREVIEW_PROXY_PORT", "HOST"]}
        env.update(LVK_INSTANCE_DIR=str(instance), LVK_ACCESS_DEPLOYMENT="local", LVK_AUTH_MODE="local", HOST="127.0.0.1", BACKEND_PORT=str(backend), PREVIEW_PROXY_PORT=str(proxy), VIBE_KANBAN_AGENT_PROCESS_HOST=str(ROOT / "target/debug/agent-process-host"), CODEX_HOME=str(home), DISABLE_WORKTREE_CLEANUP="1", RUST_LOG="warn")
        process = None
        csrf = None

        def api(path, body=None, method=None):
            headers = {"Content-Type": "application/json", "Origin": f"http://127.0.0.1:{backend}"}
            if csrf:
                headers["x-lvk-csrf"] = csrf
            request = urllib.request.Request(f"http://127.0.0.1:{backend}{path}", data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
            try:
                with urllib.request.urlopen(request, timeout=10) as response:
                    result = json.load(response)
            except urllib.error.HTTPError as error:
                raise AssertionError(f"{path}: HTTP {error.code} {error.read().decode()}") from error
            assert result.get("success", True), result
            return result.get("data", result)

        def read_agent(agent):
            try:
                return api(f"/api/agent-runs/{agent}")
            except AssertionError as error:
                if "was not found" in str(error):
                    return {"status": "starting"}
                raise

        def git(*args):
            return subprocess.check_output(["git", "-C", str(repo), *args], stderr=subprocess.PIPE, text=True).strip()

        try:
            with (root / "server.log").open("w") as log:
                process = subprocess.Popen([str(ROOT / "target/debug/server")], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                def ready():
                    assert process.poll() is None, (root / "server.log").read_text()[-4000:]
                    try:
                        return api("/api/access/status")
                    except OSError:
                        return None
                access = poll(ready, bool)
                csrf = access["csrf"]
                git("init", "-b", "main")
                for key in ["user.name", "user.email"]:
                    value = subprocess.check_output(["git", "config", key], cwd=ROOT, text=True).strip()
                    git("config", key, value)
                (repo / "fixture").write_text("base\n")
                (repo / ".lvk").mkdir()
                (repo / ".lvk/.gitignore").write_text("/shared/\n/attachments/\n")
                git("add", "fixture", ".lvk/.gitignore")
                git("commit", "-m", "approval fixture base")
                base = git("rev-parse", "HEAD")
                registered = api("/api/repos", {"path": str(repo), "display_name": "approval-fixture"})
                api(f'/api/repos/{registered["id"]}/memory', {"enabled": False, "target_branch": "main", "output_language": "en"}, "PUT")
                org = api("/api/local/v1/organizations")["organizations"][0]["id"]
                project = api("/api/local/v1/projects", {"organization_id": org, "name": "Approval fixture", "key": "APR", "color": "blue"})
                statuses = api(f'/api/board/project-statuses?project_id={project["id"]}')["project_statuses"]
                status = statuses[0]["id"]
                for policy, variant, expected in [("on-request", "DEFAULT", "on-request"), ("never", "DEFAULT", "never"), (None, "DEFAULT", "on-request"), ("never", "AUTO", "on-request"), ("on-request", "EXPLICIT", "never")]:
                    (home / "config.toml").write_text((f'approval_policy = "{policy}"\n' if policy else "") + base_config)
                    offline.Responses.issued = False
                    before = wire.stat().st_size if wire.exists() else 0
                    normal = api("/api/workspaces/start", {"name": "approval fixture", "repos": [{"repo_id": registered["id"], "target_branch": "main"}], "executor_config": {"executor": "CODEX", "variant": variant}, "prompt": "Call get_context once, then finish. Do not modify any file.", "linked_issue": None, "attachment_ids": []})
                    agent = normal["agent_run"]["agent_run_id"]
                    state = poll(lambda: read_agent(agent), lambda r: r["status"] in ["succeeded", "failed", "crashed", "cancelled"])
                    assert state["status"] == "succeeded", state["status"]
                    verify_wire(wire, before, policy, "normal", expected, variant)
                    workspace = api(f'/api/workspaces/{normal["workspace"]["id"]}')
                    # A disposable source commit supplies an unmerged selection.
                    checkout = Path(workspace["container_ref"]) / registered["name"]
                    (checkout / "source").write_text("fixture source\n")
                    subprocess.run(["git", "-C", str(checkout), "add", "source"], check=True)
                    subprocess.run(["git", "-C", str(checkout), "commit", "-m", "approval fixture source"], check=True, stdout=subprocess.DEVNULL)
                    source = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()
                    issue = api("/api/board/issues", {"project_id": project["id"], "status_id": status, "title": "approval source", "description": "Fixture only", "sort_order": 0, "extension_metadata": {}})["data"]
                    api(f'/api/workspaces/{workspace["id"]}/links', {"project_id": project["id"], "issue_id": issue["id"]})
                    offline.Responses.issued = False
                    before = wire.stat().st_size
                    integration = api("/api/integrations", {"request_id": str(uuid.uuid4()), "project_id": project["id"], "repository_id": registered["id"], "target_branch": "main", "selections": [{"card_id": issue["id"], "workspace_id": workspace["id"], "expected_commit": source}], "executor_config": {"executor": "CODEX", "variant": variant}, "skip_validation": True, "instructions": "Call get_context once, then finish. Never call integration_action. Do not modify files or publish."})
                    run = poll(lambda: api(f'/api/integrations/{integration["id"]}'), lambda r: r.get("agent_run_id") is not None or r["status"] == "blocked")
                    assert run.get("agent_run_id"), run.get("error")
                    agent = run["agent_run_id"]
                    state = poll(lambda: read_agent(agent), lambda r: r["status"] in ["succeeded", "failed", "crashed", "cancelled"])
                    assert state["status"] == "succeeded", state["status"]
                    verify_wire(wire, before, policy, "formal-auto-merge", expected, variant)
                    api(f'/api/integrations/{integration["id"]}/cancel', {})
                    poll(lambda: api(f'/api/integrations/{integration["id"]}'), lambda r: r["status"] in ["cancelled", "failed", "stopped"])
                    assert git("rev-parse", "HEAD") == base, "fixture target changed"
                starts = [json.loads(line) for line in (root / "input.jsonl").read_text().splitlines()]
                assert len(starts) == 10, starts
                assert [start["approvalPolicy"] for start in starts] == [None] * 6 + ["on-request"] * 2 + ["never"] * 2, starts
                assert all(start["sandbox"] == "workspace-write" for start in starts), starts
                print("10 fresh Run launch requests verified; target unchanged", flush=True)
        finally:
            # Terminate only this disposable instance's managed processes.
            marker = f"LVK_INSTANCE_DIR={instance}".encode()
            owned = []
            for entry in Path("/proc").iterdir():
                if entry.name.isdigit():
                    try:
                        if marker in (entry / "environ").read_bytes().split(b"\0"):
                            owned.append(int(entry.name))
                    except OSError:
                        pass
            for pid in owned:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            if process:
                process.wait(timeout=15)
            model.shutdown()
            model.server_close()
            model_thread.join(timeout=5)
            poll(lambda: [pid for pid in owned if Path(f"/proc/{pid}").exists()], lambda p: not p, seconds=15)


def verify_wire(wire, offset, policy, kind, expected, variant):
    with wire.open() as file:
        file.seek(offset)
        events = [json.loads(line) for line in file]
    effective = [e["result"]["approvalPolicy"] for e in events if "approvalPolicy" in e.get("result", {})]
    assert effective == [expected], effective
    calls = [e for e in events if e.get("method") == "item/completed" and e.get("params", {}).get("item", {}).get("type") == "mcpToolCall" and e["params"]["item"].get("tool") == "get_context"]
    assert calls and calls[-1]["params"]["item"]["status"] == "completed", "get_context did not complete"
    assert not any(e.get("method") == "mcpServer/elicitation/request" for e in events)
    print(json.dumps({"kind": kind, "file": policy, "lvk_variant": variant, "effective": effective[0], "get_context": "completed"}), flush=True)


if __name__ == "__main__":
    main()
