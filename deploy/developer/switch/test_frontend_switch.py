"""Transaction faults with isolated files; never opens a real DB or signals processes."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import frontend_bundle as f
import frontend_switch as switch
import start_support as s


def artifact(root, digit, revision=None):
    dist = root / ('dist-' + digit)
    (dist / 'assets').mkdir(parents=True)
    (dist / '.vite').mkdir()
    sha = revision or digit * 40
    (dist / 'assets/chunk.js').write_text('/* chunk ' + digit + ' */')
    (dist / 'index.html').write_text(f'<h1>Frontend {digit}</h1><script type="module" src="{f.PREFIX}{sha}/assets/chunk.js"></script>')
    (dist / '.vite/manifest.json').write_text(json.dumps({'index.html': {'file': 'assets/chunk.js', 'isEntry': True}}))
    output = root / ('bundle-' + digit)
    f.package(dist, sha, output)
    return output


class FrontendSwitchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.instance = self.root / 'instance'
        self.instance.mkdir()
        self.a = artifact(self.root, 'a')
        self.b = artifact(self.root, 'b')
        manifest = f.install(self.instance, self.a)
        self.previous = {'release': '/unchanged/backend', 'manifest': {'files': {'server': 'same-hash'}}, 'frontend': manifest}
        s.save(self.instance / 'current.json', self.previous)
        # Sentinels stand in for existing state; the transaction must not open or change them.
        (self.instance / 'data').mkdir()
        (self.instance / 'data/db').write_text('untouched DB')
        (self.instance / 'run').write_text('untouched Run')
        self.identity = {'pid': 123, 'start': '456', 'boot': 'same'}
        for patcher in [
            patch.object(switch.b, 'configure'),
            patch.object(switch.b, 'DEV', self.instance),
            patch.object(switch.b, 'CONFIG', {'environment': {'LVK_DEV_EXTERNAL_FRONTEND': '1'}}, create=True),
            patch.object(switch.b, 'observation', return_value=self.identity),
            patch.object(switch.b, 'current', side_effect=lambda: s.readjson(self.instance / 'current.json')),
            patch.object(f, 'health'),
        ]:
            patcher.start()
            self.addCleanup(patcher.stop)
        self.addCleanup(self.make_writable)

    def make_writable(self):
        for p in self.root.rglob('*'):
            if p.is_dir():
                p.chmod(0o700)

    def test_switch_rollback_and_unchanged_backend(self):
        result = switch.switch(self.instance, self.b, 'UI-only change, compatible API')
        current = s.readjson(self.instance / 'current.json')
        self.assertEqual(result['process'], self.identity)
        self.assertEqual(current['release'], self.previous['release'])
        self.assertEqual(current['manifest'], self.previous['manifest'])
        self.assertEqual(current['frontend']['revision'], 'b' * 40)
        self.assertTrue((self.instance / 'frontend-releases' / ('a' * 40) / 'assets/chunk.js').exists())
        switch.switch(self.instance, self.a, 'Rollback to retained dist')
        self.assertEqual(s.readjson(self.instance / 'current.json'), self.previous)
        self.assertEqual((self.instance / 'data/db').read_text(), 'untouched DB')
        self.assertEqual((self.instance / 'run').read_text(), 'untouched Run')

    def test_health_failure_restores_selection(self):
        with patch.object(f, 'health', side_effect=s.Refuse('HTTP mismatch')):
            with self.assertRaises(s.Refuse):
                switch.switch(self.instance, self.b, 'Compatible frontend')
        self.assertEqual(s.readjson(self.instance / 'current.json'), self.previous)
        results = list((self.instance / 'frontend-operations').glob('*/result.json'))
        self.assertTrue(s.readjson(results[0])['restored_previous'])

    def test_invalid_candidate_never_activated(self):
        (self.b / 'assets/chunk.js').chmod(0o600)
        (self.b / 'assets/chunk.js').write_text('corrupt')
        with self.assertRaises(s.Refuse):
            switch.switch(self.instance, self.b, 'Compatible frontend')
        self.assertEqual(s.readjson(self.instance / 'current.json'), self.previous)

    def test_concurrent_backend_switch_refused(self):
        with (self.instance / 'start.lock').open('a') as lock:
            s.fcntl.flock(lock, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                switch.switch(self.instance, self.b, 'Compatible frontend')
        self.assertEqual(s.readjson(self.instance / 'current.json'), self.previous)

    def test_unsettled_backend_operation_refused(self):
        op = self.instance / 'switch-operations/fixture'
        op.mkdir(parents=True)
        s.save(op / 'result.json', {'status': 'failed'})
        with self.assertRaises(s.Refuse):
            switch.switch(self.instance, self.b, 'Compatible frontend')


if __name__ == '__main__':
    unittest.main()
