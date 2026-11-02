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
    connection = parser.add_mutually_exclusive_group(required=True)
    connection.add_argument("--claude-mcp-url", help="Include an explicitly selected Claude MCP connection")
    connection.add_argument("--claude-without-connection", action="store_true",
                            help="Bundle only the Claude Skill plugin; use the user's existing LVK connector")
    args = parser.parse_args()
    from urllib.parse import urlsplit
    if args.claude_mcp_url:
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
        if client == "claude" and args.claude_without_connection:
            # An allowlist prevents future connection/config files leaking into this variant.
            manifest = ".claude-plugin/plugin.json"
            files = {manifest: files[manifest], **skill}
            assert set(json.loads(files[manifest])) <= {"name", "version", "description"}
        if client == "claude" and args.claude_mcp_url:
            files[".mcp.json"] = (json.dumps({"mcpServers": {"lvk": {"type": "http", "url": args.claude_mcp_url}}}, indent=2) + "\n").encode()
        files["distribution-source.json"] = (json.dumps({"source_commit": commit, "client": client}, indent=2) + "\n").encode()
        variant = "-without-connection" if client == "claude" and args.claude_without_connection else ""
        output = args.output / f"lvk-orchestration-{client}{variant}-{commit[:12]}.zip"
        with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
            for name, body in sorted(files.items()):
                info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, body)
        with zipfile.ZipFile(output) as archive:
            assert archive.testzip() is None
            assert set(archive.namelist()) == set(files)
            assert all(archive.read(name) == body for name, body in files.items())
            if client == "claude" and args.claude_without_connection:
                assert set(archive.namelist()) == set(skill) | {".claude-plugin/plugin.json", "distribution-source.json"}
        receipt["artifacts"][client] = {"path": str(output), "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "files": sorted(files)}
    (args.output / "distribution-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
