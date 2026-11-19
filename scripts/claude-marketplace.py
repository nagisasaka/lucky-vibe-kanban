#!/usr/bin/env python3
"""Sync the Claude Git distribution, or check it without connecting to any service."""
import argparse
import json
from pathlib import Path
import posixpath
import re
import subprocess
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
PLUGIN = "plugins/lvk-orchestration/claude/"
SKILL = "skills/lvk-orchestration/"
MARKETPLACE = ".claude-plugin/marketplace.json"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def read_files(ref=None):
    """Read only relevant files, rejecting links and executable distribution files."""
    prefixes = [MARKETPLACE, PLUGIN, SKILL]
    if ref:
        tree = git("rev-parse", "--verify", ref + "^{tree}").decode().strip()
        files = {}
        for entry in git("ls-tree", "-rz", tree, "--", *prefixes).split(b"\0"):
            if not entry:
                continue
            meta, path = entry.decode().split("\t", 1)
            mode, kind, oid = meta.split()
            require(mode == "100644" and kind == "blob", f"Not a plain file: {path}")
            files[path] = git("cat-file", "blob", oid)
        return files
    files = {}
    for prefix in prefixes:
        root = ROOT / prefix
        require(not root.is_symlink(), f"Symlink: {prefix}")
        for path in ([root] if root.is_file() else root.rglob("*")):
            require(not path.is_symlink(), f"Symlink: {path}")
            if path.is_file():
                require(not path.stat().st_mode & 0o111, f"Executable file: {path}")
                files[path.relative_to(ROOT).as_posix()] = path.read_bytes()
    return files


def validate(files):
    marketplace = json.loads(files[MARKETPLACE])
    require(marketplace["name"] == "lucky-vibe-kanban", "Unexpected marketplace name")
    require(bool(marketplace["owner"]["name"]), "Missing marketplace owner")
    entries = marketplace["plugins"]
    require(len(entries) == 1, "Expected one LVK plugin")
    entry = entries[0]
    require(entry["source"] == "./" + PLUGIN.rstrip("/"), "Unexpected plugin source")
    require(set(entry) <= {"name", "source", "description"}, "Unexpected entry configuration")
    manifest = json.loads(files[PLUGIN + ".claude-plugin/plugin.json"])
    require(entry["name"] == manifest["name"] == "lvk-orchestration", "Plugin name mismatch")
    require(set(manifest) == {"name", "version", "description"}, "Unexpected manifest configuration")
    require(bool(manifest["version"]) and bool(manifest["description"]), "Missing plugin metadata")
    require(json.loads(files[PLUGIN + ".mcp.json"]) == {
        "mcpServers": {"lvk": {"type": "http", "url": "https://luckyvibekanban.com/mcp"}}
    }, "MCP configuration must contain only the existing public HTTP endpoint")
    source = {name: body for name, body in files.items() if name.startswith(SKILL)}
    require(SKILL + "SKILL.md" in source, "Missing canonical SKILL.md")
    frontmatter = source[SKILL + "SKILL.md"].decode().split("---", 2)
    require(len(frontmatter) == 3 and not frontmatter[0], "Missing Skill frontmatter")
    require("\nname: lvk-orchestration\n" in frontmatter[1], "Skill name mismatch")
    require("\ndescription: " in frontmatter[1], "Missing Skill description")
    expected = {PLUGIN + name: body for name, body in source.items()}
    actual = {name: body for name, body in files.items() if name.startswith(PLUGIN + "skills/")}
    require(actual == expected, "Skill snapshot differs; run python3 scripts/claude-marketplace.py --sync")
    allowed = set(expected) | {PLUGIN + ".claude-plugin/plugin.json", PLUGIN + ".mcp.json"}
    require({name for name in files if name.startswith(PLUGIN)} == allowed,
            "Unexpected plugin files (only manifest, MCP config and generated Skill are distributed)")
    for name, body in source.items():
        require(name.endswith(".md"), f"Unexpected Skill asset; review before distributing: {name}")
        for link in re.findall(r"\]\(([^)]+)\)", body.decode()):
            parsed = urlsplit(link)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            target = posixpath.normpath(posixpath.join(posixpath.dirname(name), unquote(parsed.path)))
            require(target in source, f"Missing or external Skill reference: {name}: {link}")
    return len(source)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--sync", action="store_true", help="Refresh the committed Skill snapshot from its canonical source")
    mode.add_argument("--check", action="store_true", help="Validate distribution contents and canonical equality")
    parser.add_argument("--ref", help="Check an immutable Git tree (HEAD or a tree ID), not working files")
    args = parser.parse_args()
    if args.sync and args.ref:
        parser.error("--ref is only valid with --check")
    files = read_files(args.ref)
    if args.sync:
        source = {name: body for name, body in files.items() if name.startswith(SKILL)}
        require(SKILL + "SKILL.md" in source, "Missing canonical SKILL.md")
        expected = {PLUGIN + name: body for name, body in source.items()}
        # Validate everything before changing the generated snapshot.
        candidate = {name: body for name, body in files.items() if not name.startswith(PLUGIN + "skills/")}
        candidate.update(expected)
        validate(candidate)
        for name in files:
            if name.startswith(PLUGIN + "skills/") and name not in expected:
                (ROOT / name).unlink()
        for name, body in expected.items():
            target = ROOT / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
        files = read_files()
    count = validate(files)
    print(f"Claude Git distribution valid: {count} canonical Skill files; source={PLUGIN.rstrip('/')}; ref={args.ref or 'working tree'}")


if __name__ == "__main__":
    main()
