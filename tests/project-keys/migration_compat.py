"""SQL/checksum model only; real Rust/SQLx startup tests require Cargo release."""
import hashlib
import pathlib
import re
import sqlite3
import unittest
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORIGINAL = (ROOT / "crates/db/migrations/20261025120000_project_keys.sql").read_bytes()
COMPAT = (ROOT / "crates/db/migration_compat/project_keys_multi.sql").read_bytes()
OLD_HASH = hashlib.sha384(ORIGINAL).digest()
NEW_HASH = hashlib.sha384(COMPAT).digest()
VERSION = 20261025120000


def fixture(count):
    db = sqlite3.connect(":memory:")
    # Deliberately minimal: these are not the full 115 historical migrations.
    db.executescript("""
        PRAGMA foreign_keys=ON;
        CREATE TABLE projects(id BLOB PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE local_issues(id INTEGER PRIMARY KEY, project_id BLOB,
            issue_number INTEGER, simple_id TEXT,
            FOREIGN KEY(project_id) REFERENCES projects(id),
            UNIQUE(project_id, issue_number), UNIQUE(project_id, simple_id));
        CREATE TABLE _sqlx_migrations(version INTEGER PRIMARY KEY, checksum BLOB);
        CREATE TABLE prior_committed_state(value TEXT);
        INSERT INTO prior_committed_state VALUES('classification retained');
    """)
    db.executemany("INSERT INTO projects VALUES(?, 'Same name')",
                   ((uuid.UUID(int=n).bytes,) for n in range(count, 0, -1)))
    db.executemany("INSERT INTO local_issues(project_id,issue_number,simple_id) VALUES(?,1,'LOCAL-1')",
                   ((uuid.UUID(int=n).bytes,) for n in range(1, count + 1)))
    db.commit()
    return db


def apply(db, sql):
    try:
        db.executescript("BEGIN;\n" + sql.decode())
        db.execute("INSERT INTO _sqlx_migrations VALUES(?,?)",
                   (VERSION, hashlib.sha384(sql).digest()))
        db.commit()
    except Exception:
        db.rollback()
        raise


def resume(db):
    # Model the Rust selector, not a replacement for its tests.
    receipt = db.execute("SELECT checksum FROM _sqlx_migrations WHERE version=?", (VERSION,)).fetchone()
    if receipt is None:
        apply(db, COMPAT)
    elif receipt[0] not in (OLD_HASH, NEW_HASH):
        raise ValueError("unknown checksum")


class CompatibilityTests(unittest.TestCase):
    def test_checksums_match_frozen_rust_constants(self):
        source = (ROOT / "crates/db/src/migration_compat.rs").read_text()
        for name, checksum in [("ORIGINAL_CHECKSUM", OLD_HASH), ("COMPAT_CHECKSUM", NEW_HASH)]:
            body = re.search(r"const " + name + r": \[u8; 48\] = \[(.*?)\];", source, re.S)[1]
            self.assertEqual(bytes(map(int, re.findall(r"\d+", body))), checksum)

    def test_fresh_zero_one_and_multiple_are_stable_and_keep_issue_ids(self):
        for count in [0, 1, 2, 3]:
            with self.subTest(count=count), fixture(count) as db:
                before = db.execute("SELECT * FROM local_issues ORDER BY id").fetchall()
                resume(db)
                receipt = db.execute("SELECT * FROM _sqlx_migrations").fetchall()
                resume(db)
                self.assertEqual(receipt, db.execute("SELECT * FROM _sqlx_migrations").fetchall())
                keys = db.execute("SELECT key FROM projects ORDER BY hex(id)").fetchall()
                self.assertEqual(keys, [("LOCAL" if n == 1 else f"LOCAL{n}",) for n in range(1, count + 1)])
                self.assertEqual(before, db.execute("SELECT * FROM local_issues ORDER BY id").fetchall())

    def test_original_applied_zero_one_receipts_remain_unchanged(self):
        for count in [0, 1]:
            with self.subTest(count=count), fixture(count) as db:
                apply(db, ORIGINAL)
                before = db.execute("SELECT * FROM _sqlx_migrations").fetchall()
                resume(db)
                self.assertEqual(before, db.execute("SELECT * FROM _sqlx_migrations").fetchall())
                self.assertEqual(before[0][1], OLD_HASH)

    def test_failed_original_retains_prior_commit_and_can_resume(self):
        for count in [2, 3]:
            with self.subTest(count=count), fixture(count) as db:
                with self.assertRaises(sqlite3.IntegrityError):
                    apply(db, ORIGINAL)
                self.assertNotIn("key", [r[1] for r in db.execute("PRAGMA table_info(projects)")])
                self.assertEqual(db.execute("SELECT count(*) FROM _sqlx_migrations").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT value FROM prior_committed_state").fetchone()[0], "classification retained")
                resume(db)
                resume(db)
                self.assertEqual(db.execute("SELECT checksum FROM _sqlx_migrations").fetchone()[0], NEW_HASH)
                self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
                self.assertEqual(db.execute("PRAGMA foreign_key_check").fetchall(), [])

    def test_unknown_checksum_is_rejected_without_receipt_update(self):
        with fixture(1) as db:
            apply(db, ORIGINAL)
            db.execute("UPDATE _sqlx_migrations SET checksum=x'0102'")
            db.commit()
            with self.assertRaisesRegex(ValueError, "unknown checksum"):
                resume(db)
            self.assertEqual(db.execute("SELECT checksum FROM _sqlx_migrations").fetchone()[0], b"\x01\x02")

    def test_default_uniqueness_and_ten_character_limit(self):
        with fixture(0) as db:
            resume(db)
            db.execute("INSERT INTO projects(id,name) VALUES(x'01','default')")
            self.assertEqual(db.execute("SELECT key FROM projects").fetchone()[0], "LOCAL")
            for key in ["LOCAL", "LOCAL100000", "local2", "1A"]:
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("INSERT INTO projects(id,name,key) VALUES(?,?,?)", (b"two", "bad", key))
            db.execute("INSERT INTO projects(id,name,key) VALUES(x'02','limit','LOCAL99999')")

    def test_last_generated_key_and_overflow_rollback(self):
        with fixture(99999) as db:
            resume(db)
            self.assertEqual(db.execute("SELECT key FROM projects ORDER BY hex(id) DESC LIMIT 1").fetchone()[0], "LOCAL99999")
            self.assertEqual(db.execute("SELECT count(DISTINCT key) FROM projects").fetchone()[0], 99999)
        with fixture(100000) as db:
            for _ in range(2):
                with self.assertRaises(sqlite3.IntegrityError):
                    resume(db)
            self.assertNotIn("key", [r[1] for r in db.execute("PRAGMA table_info(projects)")])
            self.assertEqual(db.execute("SELECT count(*) FROM _sqlx_migrations").fetchone()[0], 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
