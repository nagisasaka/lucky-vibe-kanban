import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { randomUUID } from "node:crypto";
import { realpathSync, existsSync } from "node:fs";
import { dirname, basename, resolve, join } from "node:path";

// Seeds only this run's disposable instance. No copies of real application data.
export function seedIssueComments(instance) {
  const root = realpathSync(instance);
  assert.equal(dirname(root), realpathSync(".build"));
  assert.ok(basename(root).startsWith("issue-comments-instance-"));
  assert.ok(existsSync(join(root, "issue-comments-e2e-owner")));
  const dbPath = join(root, "data/db.v2.sqlite");
  assert.equal(realpathSync(dbPath), resolve(dbPath));
  const db = new DatabaseSync(dbPath);
  const project = randomUUID(),
    status = randomUUID(),
    issue = randomUUID();
  const comments = [randomUUID(), randomUUID()];
  const bytes = (id) => Buffer.from(id.replaceAll("-", ""), "hex");
  const revision = "2026-10-22 00:00:00.000";
  try {
    db.exec("PRAGMA foreign_keys=ON; BEGIN");
    db.prepare("INSERT INTO projects(id,name) VALUES(?,?)").run(
      bytes(project),
      "Review sharing fixture",
    );
    db.prepare(
      "INSERT INTO local_project_metadata(project_id,organization_id,color,sort_order) VALUES(?,?,?,0)",
    ).run(
      bytes(project),
      bytes("00000000-0000-0000-0000-000000000002"),
      "210 80% 52%",
    );
    db.prepare(
      "INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,?,?,0)",
    ).run(bytes(status), bytes(project), "Todo", "blue");
    db.prepare(
      "INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order,updated_at) VALUES(?,?,1,'REVIEW-1',?,?,?,0,?)",
    ).run(
      bytes(issue),
      bytes(project),
      bytes(status),
      "Review sharing fixture",
      "Agreed requirements",
      revision,
    );
    const insert = db.prepare(
      "INSERT INTO local_issue_comments(id,issue_id,author_id,message,reviewer_label,target_revision,recorded_principal) VALUES(?,?,?,?,?,?,?)",
    );
    comments.forEach((id, index) =>
      insert.run(
        bytes(id),
        bytes(issue),
        bytes("00000000-0000-0000-0000-000000000001"),
        `## Opinion ${index + 1}\nIndependent fixture review`,
        index ? "Dot" : "ChatGPT",
        revision,
        '["fixture-issuer","shared-user"]',
      ),
    );
    db.exec("COMMIT");
    return { project, issue, comments, revision };
  } finally {
    db.close();
  }
}
