"""Own all isolated artifacts/processes for the production HTTP MCP subprocess test."""

import json
import signal
import sys
import time
from pathlib import Path

from test_switch import SwitchTest

fixture = SwitchTest()
fixture.setUp()
if len(sys.argv) > 2:
    fixture.old.terminate()
    fixture.old.wait(timeout=5)
    previous = dict(fixture.builds[0], binary=str(Path(sys.argv[2]).resolve()))
    fixture.write_run(previous)
stop = False


def terminate(*_):
    global stop
    stop = True


signal.signal(signal.SIGTERM, terminate)
signal.signal(signal.SIGINT, terminate)
root = Path(sys.argv[1])
(root / "fixture.json").write_text(
    json.dumps(
        {
            "instance": str(fixture.instance),
            "config": str(fixture.config),
            "selection": fixture.builds[1],
        }
    )
)
try:
    while not stop:
        time.sleep(0.05)
finally:
    try:
        import start_binary as engine

        deadline = time.monotonic() + 175
        for op in (fixture.instance / "switch-operations").glob("*/result.json"):
            owner = json.loads((op.parent / "request.json").read_text())["owner"]
            while engine.status(op.parent.name, owner)["status"] not in engine.TERMINAL:
                if time.monotonic() > deadline:
                    raise RuntimeError("Fixture worker did not settle")
                time.sleep(0.05)
    finally:
        fixture.tearDown()
