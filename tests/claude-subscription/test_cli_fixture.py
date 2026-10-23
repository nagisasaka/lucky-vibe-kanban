"""Fixture self-checks only: these do not test LVK or a real Claude CLI."""
import json
from pathlib import Path
import subprocess
import sys
import unittest

FIXTURE = Path(__file__).with_name("cli_fixture.py")


def invoke(args, messages=()):
    result = subprocess.run(
        [sys.executable, str(FIXTURE), *args],
        input="".join(json.dumps(message) + "\n" for message in messages),
        text=True, capture_output=True, timeout=5,
    )
    return result.returncode, [json.loads(line) for line in result.stdout.splitlines()]


class FixtureTest(unittest.TestCase):
    def test_auth_status_is_non_secret_metadata(self):
        code, messages = invoke(["--settings", '{"forceLoginMethod":"claudeai"}', "auth", "status"])
        self.assertEqual(code, 0)
        self.assertEqual(messages, [{"loggedIn": True, "authMethod": "claude.ai", "apiProvider": "firstParty"}])

    def test_start_and_resume(self):
        for args in [[], ["--resume", "prior-session"]]:
            code, messages = invoke(["-p", *args], [{"type": "user", "message": {"content": "run"}}])
            self.assertEqual(code, 0)
            self.assertEqual(messages[-1]["type"], "result")
            self.assertFalse(messages[-1]["is_error"])
            if args:
                self.assertEqual(messages[0]["session_id"], "prior-session")

    def test_failure(self):
        code, messages = invoke(["-p"], [{"type": "user", "message": {"content": "CLAUDE_FIXTURE_FAIL"}}])
        self.assertEqual(code, 1)
        self.assertTrue(messages[-1]["is_error"])

    def test_interrupt(self):
        code, messages = invoke(["-p"], [
            {"type": "user", "message": {"content": "CLAUDE_FIXTURE_HOLD"}},
            {"type": "control_request", "request_id": "cancel", "request": {"subtype": "interrupt"}},
        ])
        self.assertEqual(code, 0)
        self.assertEqual(messages[-1]["result"], "Fixture interrupted")


if __name__ == "__main__":
    unittest.main()
