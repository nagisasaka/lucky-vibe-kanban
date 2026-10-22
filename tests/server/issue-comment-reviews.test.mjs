import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

const migration = "20261023100000_issue_comment_reviews.sql";
const migrations = readdirSync("crates/db/migrations")
  .filter((name) => name.endsWith(".sql"))
  .sort();
const apply = (db, name) =>
  db.exec(
    `BEGIN;${readFileSync(`crates/db/migrations/${name}`, "utf8")};COMMIT;`,
  );

test("review migration preserves existing comments and ordinary writes without revising requirements", () => {
  const db = new DatabaseSync(":memory:");
  try {
    for (const name of migrations.filter((name) => name < migration))
      apply(db, name);
    db.exec(`
      INSERT INTO projects(id,name) VALUES (x'01','fixture');
      INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(x'02',x'01','Todo','blue',0);
      INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order)
        VALUES(x'03',x'01',1,'TEST-1',x'02','Agreed title','Agreed requirements',0);
      INSERT INTO local_issue_comments(id,issue_id,message) VALUES(x'04',x'03','Existing ordinary comment');
    `);
    const before = db.prepare("SELECT * FROM local_issues").get();
    apply(db, migration);
    const existing = db.prepare("SELECT * FROM local_issue_comments").get();
    assert.equal(existing.message, "Existing ordinary comment");
    for (const field of [
      "reviewer_label",
      "target_revision",
      "recorded_principal",
    ])
      assert.equal(existing[field], null);
    const insert = db.prepare(
      "INSERT INTO local_issue_comments(id,issue_id,message,reviewer_label,target_revision,recorded_principal) VALUES(?,x'03',?,?,?,?)",
    );
    insert.run(
      Buffer.from([5]),
      "Review A",
      "ChatGPT",
      before.updated_at,
      '["fixture-issuer","shared-user"]',
    );
    insert.run(
      Buffer.from([6]),
      "Review B",
      "Dot",
      before.updated_at,
      '["fixture-issuer","shared-user"]',
    );
    db.exec(
      "INSERT INTO local_issue_comments(id,issue_id,message) VALUES(x'07',x'03','New ordinary comment')",
    );
    assert.equal(
      db.prepare("SELECT count(*) AS n FROM local_issue_comments").get().n,
      4,
    );
    assert.deepEqual(db.prepare("SELECT * FROM local_issues").get(), before);
    db.exec(
      "UPDATE local_issues SET description='Revised',updated_at='2099-01-01 00:00:00' WHERE id=x'03'",
    );
    insert.run(
      Buffer.from([8]),
      "Late review",
      "Claude",
      before.updated_at,
      '["fixture-issuer","shared-user"]',
    );
    assert.equal(
      db
        .prepare(
          "SELECT target_revision FROM local_issue_comments WHERE id=x'08'",
        )
        .get().target_revision,
      before.updated_at,
    );
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally {
    db.close();
  }
});
