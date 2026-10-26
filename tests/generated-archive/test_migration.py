"""Isolated SQL contract tests; no LVK database or managed Workspace access."""
import json
from pathlib import Path
import sqlite3
import unittest

MIGRATION = Path(__file__).resolve().parents[2] / "crates/db/migrations/20261026010000_generated_archive.sql"


class GeneratedArchiveMigration(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.executescript("""
            PRAGMA foreign_keys=ON;
            CREATE TABLE workspaces(id BLOB PRIMARY KEY, archived INTEGER DEFAULT 0, execution_owner TEXT);
            CREATE TABLE local_issues(id BLOB PRIMARY KEY, archived INTEGER DEFAULT 0);
            CREATE TABLE local_workspace_links(workspace_id BLOB, issue_id BLOB);
        """)
        self.db.executescript(MIGRATION.read_text())
        self.db.execute("INSERT INTO workspaces(id) VALUES('workspace')")
        self.db.execute("INSERT INTO local_issues(id) VALUES('issue')")
        self.db.execute("INSERT INTO local_workspace_links VALUES('workspace','issue')")
        self.db.execute("""INSERT INTO generated_issue_workspaces
            (workspace_id,issue_id,project_id,owner_kind,repository_id)
            VALUES('workspace','issue','project','openwiki_sync','repo')""")

    def result(self):
        return self.db.execute("SELECT owner_run_id,attempt_result FROM generated_issue_workspaces").fetchone()

    def test_first_owner_bind_is_immutable(self):
        for run in ("first", "replacement"):
            self.db.execute("UPDATE workspaces SET execution_owner=?", (json.dumps({"kind": "openwiki_sync", "run_id": run}),))
        self.assertEqual(self.result(), ("first", None))

    def test_manual_unarchive_consumes_eligibility(self):
        for table in ("local_issues", "workspaces"):
            with self.subTest(table=table):
                self.db.execute("UPDATE generated_issue_workspaces SET attempt_result=NULL")
                self.db.execute(f"UPDATE {table} SET archived=1")
                self.db.execute(f"UPDATE {table} SET archived=0")
                self.assertIsNotNone(self.result()[1])

    def test_unlink_relink_aba_consumes_eligibility(self):
        self.db.execute("DELETE FROM local_workspace_links")
        self.db.execute("INSERT INTO local_workspace_links VALUES('workspace','issue')")
        self.assertIsNotNone(self.result()[1])

    def test_attempt_claim_is_one_shot(self):
        statement = "UPDATE generated_issue_workspaces SET attempt_result='started' WHERE workspace_id='workspace' AND attempt_result IS NULL"
        self.assertEqual(self.db.execute(statement).rowcount, 1)
        self.assertEqual(self.db.execute(statement).rowcount, 0)

    def test_existing_explicit_delete_can_remove_provenance(self):
        self.db.execute("DELETE FROM workspaces")
        self.assertIsNone(self.result())
        self.db.execute("INSERT INTO workspaces(id) VALUES('workspace')")
        self.db.execute("""INSERT INTO generated_issue_workspaces
            (workspace_id,issue_id,project_id,owner_kind,repository_id)
            VALUES('workspace','issue','project','integration','repo')""")
        self.db.execute("DELETE FROM local_issues")
        self.assertIsNone(self.result())


if __name__ == "__main__":
    unittest.main()
