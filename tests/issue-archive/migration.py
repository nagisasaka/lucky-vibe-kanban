"""Migration contracts on a disposable in-memory DB; never opens application data."""
import pathlib
import sqlite3
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

class IssueArchiveMigration(unittest.TestCase):
    def test_existing_rows_and_workspace_archive_are_independent(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        # Minimal pre-migration tables isolate defaults/triggers from host
        # SQLite's datetime(subsec) support. Rust fixtures run all migrations.
        db.executescript("""
          CREATE TABLE projects(id BLOB PRIMARY KEY, name TEXT);
          CREATE TABLE local_project_statuses(id BLOB PRIMARY KEY,project_id BLOB,name TEXT,color TEXT,sort_order INT);
          CREATE TABLE local_issues(id BLOB PRIMARY KEY,project_id BLOB,issue_number INT,simple_id TEXT,status_id BLOB,title TEXT);
          CREATE TABLE workspaces(id BLOB PRIMARY KEY,branch TEXT,archived INT DEFAULT 0);
          CREATE TABLE local_workspace_links(workspace_id BLOB PRIMARY KEY,project_id BLOB,issue_id BLOB);
        """)
        db.execute("INSERT INTO projects(id,name) VALUES(?, 'fixture')", (b'p',))
        db.execute("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Cancelled','gray',500)",(b's',b'p'))
        db.execute("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title) VALUES(?,?,1,'TEST-1',?,'retained')",(b'i',b'p',b's'))
        db.execute("INSERT INTO workspaces(id,branch,archived) VALUES(?,'retained',1)",(b'w',))
        db.executescript((ROOT/'crates/db/migrations/20261025150000_issue_archive.sql').read_text())
        self.assertEqual(db.execute('SELECT archived,status_id FROM local_issues').fetchone(),(0,b's'))
        self.assertEqual(db.execute('SELECT archived FROM workspaces').fetchone(),(1,))
        db.execute('UPDATE local_issues SET archived=1')
        self.assertEqual(db.execute('SELECT status_id FROM local_issues').fetchone(),(b's',))
        with self.assertRaisesRegex(sqlite3.IntegrityError,'Issue is archived'):
            db.execute('INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)',(b'w',b'p',b'i'))

if __name__ == '__main__':
    unittest.main()
