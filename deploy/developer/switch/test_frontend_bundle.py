import json
from pathlib import Path
import tempfile
import unittest

import frontend_bundle as f
import start_support as s


class FrontendBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.dist = self.root / 'dist'
        (self.dist / 'assets').mkdir(parents=True)
        (self.dist / '.vite').mkdir()
        self.sha = 'a' * 40
        (self.dist / 'assets/main.js').write_text('import("./lazy.js")')
        (self.dist / 'assets/lazy.js').write_text('export default 42')
        (self.dist / 'index.html').write_text(
            f'<script type="module" src="{f.PREFIX}{self.sha}/assets/main.js"></script>')
        (self.dist / '.vite/manifest.json').write_text(json.dumps({
            'index.html': {'file': 'assets/main.js', 'isEntry': True, 'dynamicImports': ['lazy']},
            'lazy': {'file': 'assets/lazy.js'},
        }))

    def package(self):
        return f.package(self.dist, self.sha, self.root / 'bundle')

    def test_package_and_hash_validation(self):
        manifest = self.package()
        self.assertEqual(f.inspect(self.root / 'bundle'), manifest)
        target = self.root / 'bundle/assets/lazy.js'
        target.chmod(0o600)
        target.write_text('changed')
        with self.assertRaises(s.Refuse):
            f.inspect(self.root / 'bundle')
        # Allow TemporaryDirectory to remove the immutable fixture.
        for p in (self.root / 'bundle').rglob('*'):
            if p.is_dir():
                p.chmod(0o700)
        (self.root / 'bundle').chmod(0o700)

    def test_missing_dynamic_chunk_refuses_before_copy(self):
        (self.dist / 'assets/lazy.js').unlink()
        with self.assertRaises(s.Refuse):
            self.package()
        self.assertFalse((self.root / 'bundle').exists())

    def test_wrong_base_and_external_entry_refused(self):
        for url in ['/assets/main.js', 'https://example.com/main.js']:
            (self.dist / 'index.html').write_text(f'<script type="module" src="{url}"></script>')
            with self.assertRaises(s.Refuse):
                self.package()

    def test_symlink_and_traversal_refused(self):
        (self.dist / 'assets/alias').symlink_to('/etc/hosts')
        with self.assertRaises(s.Refuse):
            self.package()
        for name in ['../secret', '/secret', 'a//b', 'a/./b', 'a\\b']:
            with self.assertRaises(s.Refuse):
                f.safe_name(name)


if __name__ == '__main__':
    unittest.main()
