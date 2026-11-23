"""Switch only the dev dist under the existing start.lock. Never stop a process."""
import argparse
import json
import os
from pathlib import Path
import time
import uuid

import frontend_bundle as f
import start_binary as b
import start_support as s


def switch(instance, bundle, reason, dry_run=False):
    b.configure(instance)
    s.need(reason.strip(), 'Record compatibility decision and its inputs')
    s.need(b.CONFIG['environment'].get('LVK_DEV_EXTERNAL_FRONTEND') == '1',
           'External frontend must first be enabled through a backend migration')
    with (b.DEV / 'start.lock').open('a') as lock:
        s.fcntl.flock(lock, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
        # Do not cross an unresolved backend cutover/recovery fence.
        operations = b.DEV / 'switch-operations'
        if operations.exists():
            for op in operations.iterdir():
                if op.is_dir() and not op.name.startswith('.pending-'):
                    s.need((op / 'result.json').exists() and
                           s.readjson(op / 'result.json').get('status') == 'succeeded',
                           'Unsettled backend operation; inspect before recovery')
        identity = b.observation()
        previous = b.current()
        f.verify_selection(b.DEV, previous)
        manifest = f.inspect(bundle)
        if dry_run:
            return {'status': 'validated', 'frontend': manifest['revision'], 'process': identity}
        start = time.monotonic()
        manifest = f.install(b.DEV, bundle)
        selected = dict(previous, frontend=manifest)
        root = b.DEV / 'frontend-operations'
        root.mkdir(mode=0o700, exist_ok=True)
        op = root / str(uuid.uuid4())
        op.mkdir(mode=0o700)
        s.save(op / 'selection.json', selected)
        s.save(op / 'previous.json', previous)
        s.save(op / 'request.json', {'bundle': str(bundle), 'reason': reason, 'process': identity})
        s.need(b.observation() == identity and b.current() == previous, 'Running selection changed')
        f.verify_selection(b.DEV, selected)
        result = {'operation': str(op), 'status': 'failed'}
        try:
            s.atomic(b.DEV / 'current.json', json.dumps(selected).encode())
            f.health(manifest)
            s.need(b.observation() == identity, 'Backend changed during frontend switch')
            result.update(status='succeeded', frontend=manifest['revision'], process=identity)
        except BaseException:
            # Only this transaction owns current under start.lock. Restore the
            # entire old selection, never binaries, DB, launch config or Runs.
            if b.current() == selected:
                s.atomic(b.DEV / 'current.json', json.dumps(previous).encode())
                result['restored_previous'] = True
            raise
        finally:
            result['switch_seconds'] = time.monotonic() - start
            s.save(op / 'result.json', result)
        return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--instance', type=Path, default=Path(__file__).resolve().parent.parent)
    p.add_argument('--bundle', type=Path, required=True)
    p.add_argument('--reason', required=True)
    p.add_argument('--dry-run', action='store_true')
    args = p.parse_args()
    os.umask(0o077)
    print(json.dumps(switch(args.instance, args.bundle.absolute(), args.reason, args.dry_run)))
