"""Package already-built, externally tested artifacts; does not build or judge migrations."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import start_support as s


def package(server, host, mcp, output):
    output.mkdir(mode=0o700)
    for name, source in [
        ("server", server),
        ("agent-process-host", host),
        ("vibe-kanban-mcp", mcp),
    ]:
        if not source.is_file() or not os.access(source, os.X_OK):
            raise ValueError("Missing executable")
        shutil.copy2(source, output / name)
        (output / name).chmod(0o500)
    result = subprocess.run(
        [str(output / "server"), "runtime-payload"],
        check=True,
        cwd=output,
        env={"PATH": "/usr/bin:/bin"},
        capture_output=True,
        timeout=20,
    )
    payload = json.loads(result.stdout)
    if payload["embedded"] is not True:
        raise ValueError("Payload must be embedded")
    manifest = {
        "format": 1,
        "assets": payload["assets"],
        "files": {
            name: s.digest(output / name)
            for name in ["server", "agent-process-host", "vibe-kanban-mcp"]
        },
    }
    s.save(output / "bundle.json", manifest)
    return manifest


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for name in ["server", "agent-host", "mcp", "output"]:
        p.add_argument("--" + name, type=Path, required=True)
    a = p.parse_args()
    os.umask(0o077)
    package(
        a.server.resolve(), a.agent_host.resolve(), a.mcp.resolve(), a.output.absolute()
    )
