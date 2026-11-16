"""Offline regression checks for the Claude Git distribution boundary."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("marketplace", Path(__file__).with_name("claude-marketplace.py"))
marketplace = importlib.util.module_from_spec(spec)
spec.loader.exec_module(marketplace)


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.files = marketplace.read_files()
        self.skill = marketplace.SKILL + "SKILL.md"
        self.snapshot = marketplace.PLUGIN + self.skill

    def test_current_distribution(self):
        self.assertGreater(marketplace.validate(self.files), 1)

    def test_canonical_edit_requires_regeneration(self):
        self.files[self.skill] += b"\nNew canonical instruction.\n"
        with self.assertRaisesRegex(ValueError, "snapshot differs"):
            marketplace.validate(self.files)

    def test_missing_bundled_reference(self):
        del self.files[marketplace.PLUGIN + marketplace.SKILL + "references/execution.md"]
        with self.assertRaisesRegex(ValueError, "snapshot differs"):
            marketplace.validate(self.files)

    def test_dangling_and_outside_links_even_when_copies_match(self):
        for link in ("references/missing.md", "../../README.md"):
            with self.subTest(link=link):
                files = dict(self.files)
                body = files[self.skill] + f"\n[reference]({link})\n".encode()
                files[self.skill] = files[self.snapshot] = body
                with self.assertRaisesRegex(ValueError, "Missing or external Skill reference"):
                    marketplace.validate(files)

    def test_rejects_local_execution_and_extra_payload(self):
        for path in ("hooks/hooks.json", "server.sh", ".env", "skills/extra/SKILL.md"):
            with self.subTest(path=path):
                files = {**self.files, marketplace.PLUGIN + path: b"unexpected"}
                with self.assertRaises(ValueError):
                    marketplace.validate(files)

    def test_mcp_endpoint_and_credentials_are_fixed(self):
        for config in (
            {"type": "http", "url": "http://localhost:3000/mcp"},
            {"type": "http", "url": "https://luckyvibekanban.com/mcp", "headers": {"Authorization": "secret"}},
            {"command": "node", "args": ["server.js"]},
        ):
            with self.subTest(config=config):
                files = dict(self.files)
                files[marketplace.PLUGIN + ".mcp.json"] = json.dumps({"mcpServers": {"lvk": config}}).encode()
                with self.assertRaisesRegex(ValueError, "MCP configuration"):
                    marketplace.validate(files)

    def test_source_must_select_the_self_contained_plugin(self):
        files = dict(self.files)
        manifest = json.loads(files[marketplace.MARKETPLACE])
        manifest["plugins"][0]["source"] = "./"
        files[marketplace.MARKETPLACE] = json.dumps(manifest).encode()
        with self.assertRaisesRegex(ValueError, "Unexpected plugin source"):
            marketplace.validate(files)

    def test_git_tree_excludes_untracked_files_and_rejects_symlinks(self):
        with tempfile.TemporaryDirectory(prefix="lvk-claude-tree-") as temporary:
            root = Path(temporary)
            for name, body in self.files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(body)
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.STDOUT)
            git("init", "-q")
            git("add", ".")
            # A local-only file must not make a missing Git payload pass.
            reference = marketplace.PLUGIN + marketplace.SKILL + "references/execution.md"
            git("rm", "--cached", reference)
            tree = git("write-tree").decode().strip()
            with patch.object(marketplace, "ROOT", root):
                marketplace.validate(marketplace.read_files())
                with self.assertRaisesRegex(ValueError, "snapshot differs"):
                    marketplace.validate(marketplace.read_files(tree))
                git("add", reference)
                link = root / marketplace.PLUGIN / "outside"
                link.symlink_to("../../../../skills/lvk-orchestration")
                git("add", str(link))
                tree = git("write-tree").decode().strip()
                with self.assertRaisesRegex(ValueError, "Not a plain file"):
                    marketplace.read_files(tree)
                with self.assertRaisesRegex(ValueError, "Symlink"):
                    marketplace.read_files()


if __name__ == "__main__":
    unittest.main()
