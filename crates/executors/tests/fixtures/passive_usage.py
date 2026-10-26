#!/usr/bin/env python3
"""Non-model CLI double: rejects every frame outside initialize and usage reads."""
import json
import os
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(os.environ["LVK_USAGE_FIXTURE"])
if "--version" in sys.argv:
    (root / "version_pid").write_text(str(os.getpid()))
    (root / "version_cwd").write_text(os.getcwd())
    version_child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(120)"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL)
    (root / "version_child_pid").write_text(str(version_child.pid))
    if os.environ.get("LVK_USAGE_FIXTURE_MODE") == "hang_version":
        time.sleep(120)
    print("2.1.999 (Claude Code)" if os.environ.get("LVK_USAGE_FIXTURE_MODE") == "unknown_version" else "2.1.308 (Claude Code)")
    sys.exit(0)
(root / "pid").write_text(str(os.getpid()))
(root / "cwd").write_text(os.getcwd())
child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(120)"])
(root / "child_pid").write_text(str(child.pid))
mode = os.environ.get("LVK_USAGE_FIXTURE_MODE", "success")
for line in sys.stdin:
    frame = json.loads(line)
    with (root / "frames").open("a") as out:
        out.write(json.dumps(frame) + "\n")
    method = frame.get("method") or frame.get("request", {}).get("subtype")
    assert method in ("initialize", "initialized", "account/rateLimits/read", "account/usage/read", "get_usage"), method
    if method == "initialized":
        continue
    if mode == "exit":
        sys.exit(0)
    if mode == "hang" or (mode == "hang_read" and method != "initialize"):
        continue
    if method == "initialize":
        result = {"userAgent": "codex/0.164.0", "codexHome": "/fixture", "platformFamily": "unix", "platformOs": "linux"}
    elif method == "account/rateLimits/read":
        assert frame["params"] == {"excludeResetCreditDetails": True}
        result = {"rateLimits": {"primary": {"usedPercent": 23, "windowDurationMins": 61, "resetsAt": None}}, "rateLimitsByLimitId": {"other": {"secondary": {"usedPercent": 7}}}}
    elif method == "account/usage/read":
        result = {"summary": {"lifetimeTokens": None}}
    else:
        assert frame["request"]["skip_behaviors"] is True
        result = {"session": {"total_cost_usd": 0.0, "model_usage": {}}, "behaviors": None, "subscription_type": "max", "rate_limits_available": True, "rate_limits": {"five_hour": {"utilization": 23.5, "resets_at": None}}}
    if "request_id" in frame:
        # An unsolicited/old response must not satisfy a current waiter.
        print(json.dumps({"type": "control_response", "response": {"subtype": "success", "request_id": "wrong-id", "response": {"secret": "never-export"}}}), flush=True)
        response = {"type": "control_response", "response": {"subtype": "success", "request_id": frame["request_id"], "response": result}}
    else:
        response = {"id": frame["id"], "result": result}
    if (mode == "unsupported" and method != "initialize") or (mode == "unsupported_usage" and method == "account/usage/read"):
        if "request_id" in frame:
            response["response"] = {"subtype": "error", "request_id": frame["request_id"], "error": "secret-provider-error"}
        else:
            response = {"id": frame["id"], "error": {"code": -32601, "message": "secret-provider-error"}}
    print(json.dumps(response), flush=True)
# Stay anchored until the host performs owned process cleanup.
time.sleep(120)
