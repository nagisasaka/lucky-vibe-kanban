#!/usr/bin/env python3
"""Build both plugin candidates from one immutable Git commit (no registration)."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile


def git(*args):
    return subprocess.check_output(["git", *args])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--claude-mcp-url", required=True)
    args = parser.parse_args()
    from urllib.parse import urlsplit
    url = urlsplit(args.claude_mcp_url)
    if url.scheme != "https" or not url.hostname or url.username or url.password or url.query or url.fragment:
        parser.error("Use an HTTPS MCP endpoint without credentials, query or fragment")
    commit = git("rev-parse", "--verify", args.commit + "^{commit}").decode().strip()
    names = git("ls-tree", "-r", "--name-only", commit).decode().splitlines()
    skill_prefix = "skills/lvk-orchestration/"
    skill = {name: git("show", f"{commit}:{name}") for name in names if name.startswith(skill_prefix)}
    assert skill_prefix + "SKILL.md" in skill
    for name, body in skill.items():
        if name.endswith(".md"):
            for link in re.findall(r"\]\(([^)]+)\)", body.decode()):
                target = (Path(name).parent / link).as_posix()
                assert target in skill, f"Missing bundled link: {name}: {link}"
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = {"source_commit": commit, "registration": "not performed", "artifacts": {}}
    for client in ("openai", "claude"):
        prefix = f"plugins/lvk-orchestration/{client}/"
        files = {name[len(prefix):]: git("show", f"{commit}:{name}") for name in names if name.startswith(prefix)}
        files.update(skill)
        if client == "claude":
            files[".mcp.json"] = (json.dumps({"mcpServers": {"lvk": {"type": "http", "url": args.claude_mcp_url}}}, indent=2) + "\n").encode()
        files["distribution-source.json"] = (json.dumps({"source_commit": commit, "client": client}, indent=2) + "\n").encode()
        output = args.output / f"lvk-orchestration-{client}-{commit[:12]}.zip"
        with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
            for name, body in sorted(files.items()):
                info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, body)
        with zipfile.ZipFile(output) as archive:
            assert archive.testzip() is None
            assert all(archive.read(name) == body for name, body in files.items())
        receipt["artifacts"][client] = {"path": str(output), "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "files": sorted(files)}
    (args.output / "distribution-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
