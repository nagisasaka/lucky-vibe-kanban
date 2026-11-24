#!/usr/bin/env python3
"""Run already-built old/new LVK db integration fixtures in private instances."""
import argparse
from contextlib import closing
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile


def snapshot(instance):
    """Include every application row and migration checksum; quote schema identifiers."""
    def quote(name):
        return '"' + name.replace('"', '""') + '"'

    def value(item):
        return {"blob": item.hex()} if isinstance(item, bytes) else item

    with closing(sqlite3.connect(f"file:{instance / 'data/db.v2.sqlite'}?mode=ro", uri=True)) as db:
        assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
        assert not db.execute("PRAGMA foreign_key_check").fetchall()
        result = {}
        for (name,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"):
            rows = [[value(v) for v in row] for row in db.execute(f"SELECT * FROM {quote(name)}")]
            result[name] = sorted(rows, key=lambda row: json.dumps(row, sort_keys=True))
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", required=True, type=Path, help="old db integration test executable")
    parser.add_argument("--new", required=True, type=Path, help="new db integration test executable")
    parser.add_argument("--repo", required=True, type=Path, help="source repository containing start_support.py")
    parser.add_argument("--timeout", type=int, default=180, help="seconds per fixture process")
    args = parser.parse_args()
    old, new, repo = args.old.resolve(strict=True), args.new.resolve(strict=True), args.repo.resolve(strict=True)
    spec = importlib.util.spec_from_file_location("lvk_upgrade_backup", repo / "deploy/developer/switch/start_support.py")
    backup = importlib.util.module_from_spec(spec)
    # Suppress import bytecode: the source checkout is an input, not an output.
    import sys
    sys.dont_write_bytecode = True
    spec.loader.exec_module(backup)
    versions = {old: "3.46.0", new: "3.54.1"}
    assert old != new, "provide two distinct old/new executables"
    results = []
    with tempfile.TemporaryDirectory(prefix="lvk-sqlite-upgrade-") as directory:
        root = Path(directory)

        def instance(name):
            path = root / name
            path.mkdir()
            (path / ".sqlite-upgrade-fixture").touch()
            return path

        def run(binary, target, mode, title, package=None, imported=False):
            env = os.environ.copy()
            env.update(LVK_INSTANCE_DIR=str(target), LVK_UPGRADE_MODE=mode,
                       LVK_UPGRADE_TITLE=title, LVK_UPGRADE_IMPORTED=str(int(imported)),
                       LVK_UPGRADE_SQLITE_VERSION=versions[binary])
            if package is not None:
                env["LVK_UPGRADE_PACKAGE"] = str(package)
            completed = subprocess.run([str(binary), "--ignored", "--exact", "sqlite_upgrade_fixture", "--nocapture"],
                                       env=env, cwd=root, capture_output=True, text=True, timeout=args.timeout)
            if completed.returncode:
                raise RuntimeError(f"{versions[binary]} {mode}:\n{completed.stdout}\n{completed.stderr}")
            # A wrong executable can return success with zero matched tests.
            assert "1 passed" in completed.stdout, completed.stdout
            results.append(f"{versions[binary]} {mode} {target.name}: passed")

        shared = instance("old-new-old")
        run(old, shared, "seed", "original")
        initial = snapshot(shared)
        run(new, shared, "verify", "original")
        assert snapshot(shared) == initial, "upgrade changed existing rows or migration receipts"
        run(new, shared, "write", "new-written")
        updated = snapshot(shared)
        run(old, shared, "verify", "new-written")
        assert snapshot(shared) == updated, "old reopen changed updated rows or receipts"
        run(old, shared, "write", "old-written")
        updated = snapshot(shared)
        run(new, shared, "verify", "old-written")
        assert snapshot(shared) == updated, "new reopen changed old-written rows or receipts"
        results.append("whole DB rows and migration checksums preserved across reopen: passed")

        for producer, consumer, name in [(old, new, "old-to-new"), (new, old, "new-to-old")]:
            package = root / f"{name}.sqlite"
            run(producer, shared, "export", "old-written", package)
            assert snapshot(shared) == updated, "export changed source rows"
            destination = instance(name)
            run(consumer, destination, "import", "old-written", package, imported=True)
            imported_snapshot = snapshot(destination)
            run(producer, destination, "verify", "old-written", imported=True)
            assert snapshot(destination) == imported_snapshot, "imported DB differs after opposite-engine reopen"

        # Exercise the real switch backup function, never controller cutover/start/stop.
        backup_path = root / "backup-data"
        backup.backup_data(shared / "data", backup_path)
        restored = instance("restored")
        shutil.copytree(backup_path, restored / "data")
        assert snapshot(restored) == updated, "backup/restore differs from source"
        run(new, restored, "verify", "old-written")
        run(old, restored, "verify", "old-written")
        assert snapshot(restored) == updated, "restored DB changed after reopen"
        results.append(f"start_support.backup_data and old/new restored reopen: passed (Python SQLite {sqlite3.sqlite_version})")
    def digest(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()

    print(json.dumps({"checks": results, "temporary_instances_cleaned": True,
                      "binaries": {str(path): {"sha256": digest(path), "sqlite": version}
                                   for path, version in versions.items()}}, indent=2))


if __name__ == "__main__":
    main()
