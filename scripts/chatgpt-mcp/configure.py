#!/usr/bin/env python3
"""Create a self-contained, non-secret plugin distribution in a new directory."""
import argparse
import json
from pathlib import Path
import shutil
from urllib.parse import urlsplit


def configure(base_url: str, output: Path) -> Path:
    url = urlsplit(base_url)
    if (url.scheme != "https" or not url.hostname or url.username or url.password
            or url.query or url.fragment or base_url.endswith("/")
            or any(c.isspace() for c in base_url) or "\\" in base_url
            or any(part in (".", "..") for part in url.path.split("/"))
            or "%" in url.path):
        raise ValueError("Use a canonical HTTPS public base URL without credentials, query, fragment or trailing slash")
    # Never overwrite a checkout, installation or user configuration.
    output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).resolve().parents[2] / "plugins" / "lvk"
    plugin = output / "plugins" / "lvk"
    shutil.copytree(source, plugin)
    path = plugin / "mcp.json"
    config = json.loads(path.read_text())
    server = config["mcpServers"]["lvk"]
    server["url"] = base_url + "/mcp"
    path.write_text(json.dumps(config, indent=2) + "\n")
    dependency = plugin / "skills/lvk-work/agents/openai.yaml"
    # JSON strings are valid quoted YAML scalars; no manual escaping or shell use.
    text = dependency.read_text().replace('"https://lvk.example.invalid/mcp"', json.dumps(base_url + "/mcp"))
    dependency.write_text(text)
    marketplace = output / ".agents/plugins/marketplace.json"
    marketplace.parent.mkdir(parents=True)
    marketplace.write_text(json.dumps({
        "name": "lvk-local", "interface": {"displayName": "My LVK"},
        "plugins": [{"name": "lvk", "source": {"source": "local", "path": "./plugins/lvk"},
                     "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                     "category": "Productivity"}],
    }, indent=2) + "\n")
    return plugin


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-base-url", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        plugin = configure(args.public_base_url, args.output)
    except (ValueError, OSError) as error:
        parser.exit(1, f"Package not created: {error}\n")
    print(f"Created {plugin}. This does not install, publish or connect it in ChatGPT.")
