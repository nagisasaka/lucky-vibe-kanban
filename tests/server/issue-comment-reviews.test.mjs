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

const previewSql = readFileSync(
  "crates/server/src/remote_mcp/sql/comment_previews.sql",
  "utf8",
);
const chunkSql = readFileSync(
  "crates/server/src/remote_mcp/sql/comment_body_chunk.sql",
  "utf8",
);
function commentsFixture(run) {
  const db = new DatabaseSync(":memory:");
  try {
    for (const name of migrations) apply(db, name);
    db.exec(`
      INSERT INTO projects(id,name) VALUES(x'01','one'),(x'11','two');
      INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(x'02',x'01','Todo','blue',0),(x'12',x'11','Todo','blue',0);
      INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES
        (x'03',x'01',1,'ONE-1',x'02','one',0),(x'04',x'01',2,'ONE-2',x'02','sibling',0),(x'13',x'11',1,'TWO-1',x'12','foreign',0);
    `);
    run(db);
  } finally {
    db.close();
  }
}
const id = (n) => Buffer.from([n]);

test("production SQL bounds 50 previews and reconstructs long Unicode/UI comments without loss", () =>
  commentsFixture((db) => {
    const bodies = Array.from({ length: 50 }, (_, i) =>
      i === 49 ? '界😀e\u0301\0\u0001\\"\n'.repeat(5000) : "😀".repeat(16000),
    );
    const insert = db.prepare(
      "INSERT INTO local_issue_comments(id,issue_id,message) VALUES(?,x'03',?)",
    );
    const before = db
      .prepare("SELECT * FROM local_issues WHERE id=x'03'")
      .get();
    bodies.forEach((body, i) => insert.run(id(i + 32), body));
    const previews = db.prepare(previewSql).all(id(3), null, null, null, 51);
    assert.equal(previews.length, 50);
    let previewCharacters = 0;
    for (const row of previews) {
      assert.ok(row.preview_bytes.length <= 1024);
      const expected = bodies[row.id[0] - 32];
      const decoder = new TextDecoder("utf-8", { fatal: true });
      const preview = Array.from(
        decoder.decode(row.preview_bytes, { stream: true }),
      )
        .slice(0, 256)
        .join("");
      previewCharacters += Array.from(preview).length;
      assert.equal(preview, Array.from(expected).slice(0, 256).join(""));
      assert.equal(row.total_bytes, Buffer.byteLength(expected));
      const chunks = [];
      for (let offset = 0; offset < row.total_bytes; offset += 16384) {
        const chunk = db
          .prepare(chunkSql)
          .get(offset + 1, 16384, row.id, id(3), id(1)).chunk;
        assert.ok(chunk.length <= 16384);
        chunks.push(chunk);
      }
      assert.equal(Buffer.concat(chunks).toString("utf8"), expected);
    }
    assert.equal(previewCharacters, 12800);
    assert.deepEqual(
      db.prepare("SELECT * FROM local_issues WHERE id=x'03'").get(),
      before,
    );
  }));

test("production chunk SQL enforces project, issue and comment boundaries including empty/terminal reads", () =>
  commentsFixture((db) => {
    db.prepare(
      "INSERT INTO local_issue_comments(id,issue_id,message) VALUES(?,x'03',?)",
    ).run(id(32), "😀before\0end");
    const chunk = (...scope) => db.prepare(chunkSql).get(1, 16384, ...scope);
    assert.equal(
      Buffer.from(chunk(id(32), id(3), id(1)).chunk).toString(),
      "😀before\0end",
    );
    assert.equal(chunk(id(32), id(4), id(1)), undefined);
    assert.equal(chunk(id(32), id(3), id(17)), undefined);
    assert.equal(chunk(id(99), id(3), id(1)), undefined);
    assert.equal(
      db.prepare(chunkSql).get(100, 16384, id(32), id(3), id(1)).chunk.length,
      0,
    );
    db.prepare("UPDATE local_issue_comments SET message='' WHERE id=?").run(
      id(32),
    );
    assert.equal(chunk(id(32), id(3), id(1)).chunk.length, 0);
  }));

test("isolated browser fixture seeds only its guarded disposable database", async () => {
  const { mkdirSync, mkdtempSync, writeFileSync, rmSync } = await import(
    "node:fs"
  );
  const { resolve, join } = await import("node:path");
  const { seedIssueComments } = await import("./fixtures/issue-comments.mjs");
  mkdirSync(".build", { recursive: true });
  const root = mkdtempSync(resolve(".build/issue-comments-instance-"));
  try {
    mkdirSync(join(root, "data"));
    const path = join(root, "data/db.v2.sqlite");
    const db = new DatabaseSync(path);
    try {
      for (const name of migrations) apply(db, name);
    } finally {
      db.close();
    }
    assert.throws(() => seedIssueComments(root)); // no task ownership marker
    writeFileSync(
      join(root, "issue-comments-e2e-owner"),
      "disposable fixture\n",
    );
    const fixture = seedIssueComments(root);
    const check = new DatabaseSync(path, { readOnly: true });
    try {
      const rows = check
        .prepare(
          "SELECT reviewer_label,target_revision,recorded_principal FROM local_issue_comments",
        )
        .all();
      assert.equal(rows.length, 2);
      assert.deepEqual(rows.map((row) => row.reviewer_label).sort(), [
        "ChatGPT",
        "Dot",
      ]);
      assert.ok(rows.every((row) => row.target_revision === fixture.revision));
      assert.ok(
        rows.every(
          (row) =>
            row.recorded_principal === '["fixture-issuer","shared-user"]',
        ),
      );
    } finally {
      check.close();
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
