"""Build dispatch contract: unused side must not invoke its toolchain."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import start_support as s
from test_frontend_switch import artifact

spec = importlib.util.spec_from_file_location('split_build', Path(__file__).with_name('build.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SplitBuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / 'source'
        self.source.mkdir()
        self.sha = 'a' * 40
        self.commands = []
        self.addCleanup(self.make_writable)

    def make_writable(self):
        for p in self.root.rglob('*'):
            if p.is_dir(): p.chmod(0o700)

    def git(self, args, **kwargs):
        return self.sha if 'rev-parse' in args else ''

    def run_command(self, command, **kwargs):
        self.commands.append(command)
        if command[0] == 'pnpm':
            output = Path(command[command.index('--outDir') + 1])
            artifact(output.parent, 'a')
            (output.parent / 'dist-a').rename(output)

    def package(self, server, host, mcp, output, frontend):
        output.mkdir()
        return {'files': {'server': 'hash'}}

    def execute(self, mode):
        with patch.object(module.subprocess, 'check_output', side_effect=self.git), patch.object(module.subprocess, 'run', side_effect=self.run_command), patch.object(module.bundle, 'package', side_effect=self.package):
            return module.build(self.source, self.root / 'output', mode, 'Explicit compatible input assessment')

    def test_frontend_never_calls_cargo(self):
        result = self.execute('frontend')
        self.assertEqual([c[0] for c in self.commands], ['pnpm'])
        self.assertEqual(result['status'], 'succeeded')
        self.assertTrue((self.root / 'output/frontend/frontend.json').exists())

    def test_backend_never_builds_frontend(self):
        self.execute('backend')
        self.assertEqual([c[0] for c in self.commands], ['node'])
        self.assertIn('scripts/dev-cargo.mjs', self.commands[0])
        self.assertFalse((self.root / 'output/frontend').exists())

    def test_both_and_failure_not_published(self):
        self.execute('both')
        self.assertEqual([c[0] for c in self.commands], ['pnpm', 'node'])
        with patch.object(module.subprocess, 'check_output', return_value='dirty'):
            with self.assertRaises(s.Refuse):
                module.build(self.source, self.root / 'failed', 'both', 'unknown: both')
        self.assertFalse((self.root / 'failed').exists())


if __name__ == '__main__':
    unittest.main()
