#!/usr/bin/env python3
"""Offline native config precedence probe. Uses no account, model, or live LVK.

Pass a pinned Codex executable as argv[1]. Each child gets a disposable
CODEX_HOME; the caller's config and environment remain untouched.
This probes fresh App Server threads, not LVK Workspace/Auto-merge lifecycles.
"""
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading


def probe(command, policy):
    with tempfile.TemporaryDirectory(prefix="lvk-approval-config-") as directory:
        config = Path(directory) / "config.toml"
        profile_policy = "never" if policy == "on-request" else "on-request"
        content = f'approval_policy = "{policy}"\n[profiles.fixture]\napproval_policy = "{profile_policy}"\n'
        config.write_text(content)
        events = queue.Queue()
        with tempfile.TemporaryFile(mode="w+") as errors:
            process = subprocess.Popen(
                command + ["app-server"], cwd=directory,
                env={**os.environ, "CODEX_HOME": directory},
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=errors, text=True,
            )
            def read():
                for line in process.stdout:
                    events.put(json.loads(line))
            reader = threading.Thread(target=read)
            reader.start()
            sequence = 0

            def rpc(method, params, allow_error=False):
                nonlocal sequence
                sequence += 1
                process.stdin.write(json.dumps({"id": sequence, "method": method, "params": params}) + "\n")
                process.stdin.flush()
                while True:
                    event = events.get(timeout=20)
                    if event.get("id") == sequence:
                        if allow_error:
                            return event
                        assert "error" not in event, event
                        return event["result"]
                    # Never silently authorize a native approval request.
                    assert "id" not in event, event

            try:
                rpc("initialize", {"clientInfo": {"name": "lvk_approval_fixture", "version": "1"}})
                process.stdin.write('{"method":"initialized","params":{}}\n')
                process.stdin.flush()
                for profile in [False, True]:
                    for override in [None, "on-request", "never"]:
                        params = {"cwd": directory, "sandbox": "workspace-write"}
                        if profile:
                            params["config"] = {"profile": "fixture"}
                        if override is not None:
                            params["approvalPolicy"] = override
                        if profile:
                            response = rpc("thread/start", params, allow_error=True)
                            assert "legacy `profile" in response["error"]["message"], response
                            print(json.dumps({"native_profile": "fixture", "thread_override": override, "outcome": "rejected: legacy profile config unsupported"}))
                            continue
                        result = rpc("thread/start", params)
                        expected = override or (profile_policy if profile else policy)
                        assert result["approvalPolicy"] == expected, result["approvalPolicy"]
                        print(json.dumps({"file": policy, "native_profile": profile_policy if profile else None, "thread_override": override, "effective": result["approvalPolicy"]}))
                assert config.read_text() == content
            finally:
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
                reader.join(timeout=5)
                assert not reader.is_alive()


if __name__ == "__main__":
    command = sys.argv[1:] or ["codex"]
    version = subprocess.check_output(command + ["--version"], text=True).strip()
    assert version == "codex-cli 0.163.0", version
    print(version)
    for policy in ["on-request", "never"]:
        probe(command, policy)
