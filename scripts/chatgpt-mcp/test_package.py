import json
from pathlib import Path
import tempfile
import unittest

import yaml
from configure import configure


class PackageConfiguration(unittest.TestCase):
    def test_subpath_is_shared_and_existing_distribution_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "distribution"
            plugin = configure("https://lvk.example/team", output)
            server = json.loads((plugin / "mcp.json").read_text())["mcpServers"]["lvk"]
            self.assertEqual(server["url"], "https://lvk.example/team/mcp")
            dependency = yaml.safe_load((plugin / "skills/lvk-work/agents/openai.yaml").read_text())
            self.assertEqual(dependency["dependencies"]["tools"][0]["url"], server["url"])
            market = json.loads((output / ".agents/plugins/marketplace.json").read_text())
            self.assertEqual(output / market["plugins"][0]["source"]["path"], plugin)
            self.assertFalse(any(p.is_symlink() for p in output.rglob("*")))
            with self.assertRaises(FileExistsError):
                configure("https://different.example", output)
            self.assertEqual(json.loads((plugin / "mcp.json").read_text())["mcpServers"]["lvk"]["url"], server["url"])

    def test_unsafe_configuration_does_not_create_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "distribution"
            for base in ["http://lvk.example", "https://user:secret@lvk.example", "https://lvk.example?token=secret",
                         "https://lvk.example/#key", "https://lvk.example/", "https://lvk.example/a/../b",
                         "https://lvk.example/a%2fb", "https://lvk.example\\evil", "https://lvk.example/with space"]:
                with self.assertRaises(ValueError):
                    configure(base, output)
                self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
