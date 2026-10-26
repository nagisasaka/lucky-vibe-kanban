"""Isolated SQL contract tests; no LVK database or managed Workspace access."""
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

MIGRATION = Path(__file__).resolve().parents[2] / "crates/db/migrations/20261026010000_generated_archive.sql"


class GeneratedArchiveMigration(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.executescript("""
            PRAGMA foreign_keys=ON;
            CREATE TABLE workspaces(id BLOB PRIMARY KEY, archived INTEGER DEFAULT 0, execution_owner TEXT, container_ref TEXT, branch TEXT);
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

    def test_intervention_after_claim_invalidates_deletion(self):
        # Same persistent started intent as the Rust settled-callback barrier.
        for intervention in ("unarchive", "archive_unarchive", "link_aba", "owner_aba", "path_aba"):
            with self.subTest(intervention=intervention):
                self.db.execute("UPDATE generated_issue_workspaces SET attempt_result=NULL")
                revision = self.db.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0]
                self.db.execute("UPDATE generated_issue_workspaces SET attempt_result='attempt started; outcome may be uncertain'")
                if intervention == "unarchive":
                    self.db.execute("UPDATE local_issues SET archived=0")
                elif intervention == "archive_unarchive":
                    self.db.execute("UPDATE local_issues SET archived=1")
                    self.db.execute("UPDATE local_issues SET archived=0")
                elif intervention == "link_aba":
                    self.db.execute("DELETE FROM local_workspace_links")
                    self.db.execute("INSERT INTO local_workspace_links VALUES('workspace','issue')")
                elif intervention == "owner_aba":
                    self.db.execute("UPDATE workspaces SET execution_owner='{}'")
                    self.db.execute("UPDATE workspaces SET execution_owner=NULL")
                elif intervention == "path_aba":
                    self.db.execute("UPDATE workspaces SET container_ref='changed'")
                    self.db.execute("UPDATE workspaces SET container_ref=NULL")
                self.db.commit()
                self.db.execute("BEGIN IMMEDIATE")
                eligible = self.db.execute("""SELECT EXISTS(SELECT 1 FROM generated_issue_workspaces
                    WHERE workspace_id='workspace' AND eligibility_revision=?
                    AND attempt_result='attempt started; outcome may be uncertain')""", (revision,)).fetchone()[0]
                self.assertFalse(eligible)
                self.assertEqual(self.result()[1], "attempt started; outcome may be uncertain")
                self.db.rollback()

    def test_sibling_link_change_keeps_future_attempt_but_invalidates_claim(self):
        revision = self.db.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0]
        self.db.execute("INSERT INTO workspaces(id) VALUES('sibling')")
        self.db.execute("INSERT INTO local_workspace_links VALUES('sibling','issue')")
        self.assertIsNone(self.result()[1])
        updated = self.db.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0]
        self.assertGreater(updated, revision)
        self.db.execute("UPDATE generated_issue_workspaces SET attempt_result='started'")
        self.db.execute("DELETE FROM local_workspace_links WHERE workspace_id='sibling'")
        self.assertEqual(self.result()[1], "started")
        self.assertGreater(self.db.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0], updated)

    def test_uninterrupted_claim_remains_eligible_in_final_transaction(self):
        revision = self.db.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0]
        self.db.execute("UPDATE generated_issue_workspaces SET attempt_result='attempt started; outcome may be uncertain'")
        self.db.commit()
        self.db.execute("BEGIN IMMEDIATE")
        self.assertEqual(self.db.execute("""SELECT count(*) FROM generated_issue_workspaces
            WHERE eligibility_revision=? AND attempt_result='attempt started; outcome may be uncertain'""", (revision,)).fetchone()[0], 1)
        self.db.rollback()

    def test_final_write_transaction_excludes_concurrent_manual_writer(self):
        self.db.commit()
        with tempfile.TemporaryDirectory(prefix="generated-archive-sql-") as directory:
            path = str(Path(directory) / "fixture.sqlite")
            first = sqlite3.connect(path, timeout=0)
            second = sqlite3.connect(path, timeout=0)
            try:
                self.db.backup(first)
                first.execute("UPDATE generated_issue_workspaces SET attempt_result='attempt started; outcome may be uncertain'")
                first.commit()
                revision = first.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0]
                first.execute("BEGIN IMMEDIATE")
                self.assertEqual(first.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0], revision)
                with self.assertRaisesRegex(sqlite3.OperationalError, "locked"):
                    second.execute("UPDATE local_issues SET archived=0")
                second.rollback()
                first.rollback()
                second.execute("UPDATE local_issues SET archived=0")
                second.commit()
                self.assertGreater(first.execute("SELECT eligibility_revision FROM generated_issue_workspaces").fetchone()[0], revision)
            finally:
                second.close()
                first.close()

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
