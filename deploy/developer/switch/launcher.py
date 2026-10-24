"""Fixed developer launcher. launch.json is the sole persistent launch configuration."""

import json
import hashlib
import os
from pathlib import Path
import sys


def launch(instance):
    instance = Path(instance).resolve(strict=True)
    launch_bytes = (instance / "launch.json").read_bytes()
    settings = json.loads(launch_bytes)
    current = json.loads((instance / "current.json").read_text())
    release = Path(current["release"])
    if (
        release.resolve(strict=True) != release
        or release.parent != instance / "releases"
    ):
        raise ValueError("Invalid current release")
    env = dict(settings["environment"])
    env["LVK_LAUNCH_CONFIG_SHA256"] = hashlib.sha256(launch_bytes).hexdigest()
    env.update(
        LVK_INSTANCE_DIR=str(instance),
        BACKEND_PORT=str(settings["port"]),
        HOST="127.0.0.1",
        VIBE_KANBAN_AGENT_PROCESS_HOST=str(release / "agent-process-host"),
    )
    env["PATH"] = str(release) + ":" + env.get("PATH", "/usr/local/bin:/usr/bin:/bin")
    env.pop("LVK_SELF_RESTART_ENGINE", None)
    env.pop("LVK_SELF_RESTART_CONFIG", None)
    os.chdir(instance)
    (instance / "server.pid").write_text(str(os.getpid()))
    os.execve(release / "server", [str(release / "server")], env)


if __name__ == "__main__":
    launch(sys.argv[1])
