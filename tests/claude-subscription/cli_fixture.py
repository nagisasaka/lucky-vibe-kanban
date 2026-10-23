#!/usr/bin/env python3
"""Offline Claude CLI protocol fixture; never authenticates or calls a model.

Place a symlink named `claude` on the isolated test server's PATH. Do not install
this fixture on the user's PATH. State and trace paths belong to the test only.
"""
import json
import os
from pathlib import Path
import sys
import uuid


def send(value):
    print(json.dumps(value), flush=True)


def trace(event, **data):
    directory = os.environ.get("LVK_CLAUDE_FIXTURE_TRACE")
    if directory:
        with (Path(directory) / f"{os.getpid()}.jsonl").open("a") as output:
            output.write(json.dumps({"event": event, **data}) + "\n")


def main():
    args = sys.argv[1:]
    if "--version" in args:
        print("Claude Code (offline LVK fixture, not an official CLI)")
        return 0
    if "--help" in args:
        print("Offline fixture: --effort --resume --settings")
        return 0
    state_file = os.environ.get("LVK_CLAUDE_FIXTURE_STATE")
    state = json.loads(Path(state_file).read_text()) if state_file else {}
    settings = json.loads(args[args.index("--settings") + 1]) if "--settings" in args else {}
    subscription = settings.get("forceLoginMethod") == "claudeai"
    if "auth" in args and "status" in args:
        method = state.get("authMethod", "claude.ai")
        logged_in = state.get("loggedIn", True)
        trace("auth", method=method, logged_in=logged_in, subscription=subscription)
        if state.get("malformed"):
            print("fixture-invalid-status")
        else:
            send({"loggedIn": logged_in, "authMethod": method,
                  "apiProvider": state.get("apiProvider", "firstParty")})
        return state.get("exitCode", 0 if logged_in else 1)

    resume = args[args.index("--resume") + 1] if "--resume" in args else None
    session_id = resume or str(uuid.uuid4())
    trace("launch", resume=resume, session_id=session_id, subscription=subscription, discovery=args[-2:] == ["--", "/"])
    send({"type": "system", "subtype": "init", "session_id": session_id,
          "model": "offline-fixture", "tools": [], "slash_commands": [],
          "apiKeySource": "/login managed key"})
    if args[-2:] == ["--", "/"]:
        return 0

    def finish(error=False, text="Fixture completed"):
        send({"type": "result", "subtype": "error_during_execution" if error else "success",
              "is_error": error, "result": text, "errors": [text] if error else [],
              "session_id": session_id, "duration_ms": 1, "duration_api_ms": 0,
              "num_turns": 1, "total_cost_usd": 0,
              "usage": {"input_tokens": 10, "output_tokens": 5}})

    for line in sys.stdin:
        message = json.loads(line)
        if message.get("type") == "control_request":
            subtype = message["request"]["subtype"]
            trace("control", subtype=subtype)
            send({"type": "control_response", "response": {
                "subtype": "success", "request_id": message["request_id"], "response": {}}})
            if subtype == "interrupt":
                finish(True, "Fixture interrupted")
                return 0
        elif message.get("type") == "user":
            content = message.get("message", {}).get("content", "")
            # Prompt text is inspected only to choose the test scenario, never logged.
            scenario = str(content)
            trace("user", resume=bool(resume))
            send({"type": "assistant", "session_id": session_id, "uuid": str(uuid.uuid4()),
                  "message": {"id": str(uuid.uuid4()), "type": "message", "role": "assistant",
                              "model": "offline-fixture", "content": [{"type": "text",
                              "text": "Fixture resumed" if resume else "Fixture started"}]}})
            if "CLAUDE_FIXTURE_HOLD" in scenario:
                continue
            failed = "CLAUDE_FIXTURE_FAIL" in scenario
            finish(failed, "Fixture failure" if failed else "Fixture completed")
            return 1 if failed else 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
