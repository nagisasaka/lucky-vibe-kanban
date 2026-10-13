#!/usr/bin/env python3
"""Validate the package against published schemas and the real Rust tool catalog.

Validation-only dependencies: jsonschema==4.23.0, PyYAML==6.0.2.
The caller supplies downloaded official schemas; this command performs no network I/O.
"""
import argparse
import json
from pathlib import Path
import re

import jsonschema
import yaml


def validate(plugin: Path, catalog: Path, schemas: Path) -> int:
    for name in ("plugin", "mcp"):
        jsonschema.Draft202012Validator(json.loads((schemas / f"{name}.schema.json").read_text())).validate(
            json.loads((plugin / f"{name}.json").read_text()))
    config = json.loads((plugin / "mcp.json").read_text())["mcpServers"]["lvk"]
    assert config["type"] == "streamable-http"
    assert set(config) == {"type", "url"}, "Portable declaration must not contain unsupported auth fields or credentials"
    skill = plugin / "skills/lvk-work"
    metadata = yaml.safe_load((skill / "SKILL.md").read_text().split("---", 2)[1])
    assert metadata["name"] == skill.name and metadata["description"]
    dependency = yaml.safe_load((skill / "agents/openai.yaml").read_text())["dependencies"]["tools"][0]
    assert dependency["value"] == "lvk" and dependency["url"] == config["url"]
    tools = {tool["name"]: tool for tool in json.loads(catalog.read_text())}
    mentioned = set()
    for path in skill.rglob("*.md"):
        text = path.read_text()
        mentioned.update(re.findall(r"`((?:get|list|create|update|start|steer|continue|answer)_[a-z_]+)`", text))
        for link in re.findall(r"\]\(([^)]+)\)", text):
            assert not link.startswith("/"), f"Host path in skill: {link}"
            resolved = (path.parent / link).resolve()
            assert resolved.is_relative_to(plugin.resolve()), f"Reference escapes the package: {link}"
            assert resolved.is_file(), f"Missing bundled reference: {link}"
    assert mentioned <= tools.keys(), f"Unknown tools: {mentioned - tools.keys()}"
    samples = json.loads((Path(__file__).parent / "examples.json").read_text())
    for case in samples:
        tool = tools[case["tool"]]
        jsonschema.Draft202012Validator(tool["inputSchema"], format_checker=jsonschema.FormatChecker()).validate(case["arguments"])
        scopes = tool["_meta"]["securitySchemes"][0]["scopes"]
        assert scopes == case["scopes"], case["tool"]
    assert mentioned <= {case["tool"] for case in samples}, "Every skill tool needs a valid input example"
    return len(samples)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--schemas", type=Path, required=True)
    args = parser.parse_args()
    print(f"Validated package, skill references and {validate(args.plugin, args.catalog, args.schemas)} tool inputs. ChatGPT behaviour is not exercised.")
