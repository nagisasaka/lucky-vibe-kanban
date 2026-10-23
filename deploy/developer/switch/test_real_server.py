"""Optional isolated acceptance against existing, externally built LVK artifacts.

This is test setup, not part of start.sh: construct historical empty schema from
trusted migration sources and let the real server perform the final migration.
No installed instance, source artifact, or live database is modified.
"""

import argparse
import contextlib
import hashlib
import json
import secrets
import sqlite3

if sqlite3.sqlite_version_info < (3, 42):
    import pysqlite3 as sqlite3  # SQLx uses a newer bundled SQLite (subsec support).
import subprocess
import sys
from pathlib import Path

from test_switch import SwitchTest


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    p.add_argument("--source", required=True)
    p.add_argument("--agent-host", required=True)
    p.add_argument("--frontend", required=True)
    a = p.parse_args()
    fixture = SwitchTest()
    fixture.setUp()
    try:
        fixture.old.terminate()
        fixture.old.wait(timeout=10)
        fixture.db.unlink()
        migrations = sorted((Path(a.source) / "crates/db/migrations").glob("*.sql"))
        assert len(migrations) > 1
        # This schema is built ONLY in the disposable fixture. Production histories
        # are never repaired or rewritten by the switch engine.
        with contextlib.closing(sqlite3.connect(fixture.db)) as db, db:
            db.execute(
                "CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)"
            )
            for sql in migrations[:-1]:
                db.executescript("BEGIN;\n" + sql.read_text())
                version, description = sql.stem.split("_", 1)
                db.execute(
                    "INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(?,?,1,?,1)",
                    (
                        int(version),
                        description.replace("_", " "),
                        hashlib.sha384(sql.read_bytes()).digest(),
                    ),
                )
                db.commit()
        token = fixture.instance / "data/access-internal.token"
        token.write_text(secrets.token_urlsafe(32))
        token.chmod(0o600)
        fixture.write_run(
            fixture.builds[0],
            {
                "LVK_ACCESS_DEPLOYMENT": "local",
                "LVK_AUTH_MODE": "",
                "LVK_PUBLIC_URL": "",
                "PORT": "",
                "PREVIEW_PROXY_PORT": "0",
                "VK_ALLOWED_ORIGINS": f"http://127.0.0.1:{fixture.port}",
            },
        )
        fixture.old = subprocess.Popen(
            [sys.executable, str(fixture.instance / "run.py")],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        fixture.wait_port()
        fixture.builds[1] = vars(a)
        for label in ["migration_pending", "no_pending_migration"]:
            command = fixture.cli(extra=["--retain-worktree"])
            result = subprocess.run(
                command, text=True, capture_output=True, timeout=130
            )
            state = json.loads(result.stdout)
            print(json.dumps({"case": label, "result": state}), flush=True)
            if state["status"] != "succeeded":
                # Retain no secret-containing logs in the public output.
                raise AssertionError("Real LVK switch failed: " + str(state))
            op = Path(state["result_path"]).parent
            with contextlib.closing(
                sqlite3.connect(op / "data/db.v2.sqlite")
            ) as backup:
                before = backup.execute(
                    "SELECT count(*) FROM _sqlx_migrations"
                ).fetchone()[0]
            with contextlib.closing(sqlite3.connect(fixture.db)) as live:
                after = live.execute(
                    "SELECT count(*) FROM _sqlx_migrations"
                ).fetchone()[0]
            assert before == len(migrations) - (label == "migration_pending"), (
                before,
                len(migrations),
            )
            assert after == len(migrations)
        print(
            "PASS: real LVK startup applied pending migration; second cutover had no pending migration. Auth0/UI/MCP not verified."
        )
    finally:
        fixture.tearDown()


if __name__ == "__main__":
    main()
