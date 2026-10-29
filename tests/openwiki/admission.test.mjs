// Architectural regression: recovery may resume reserved owners, never admit a
// new one. Runtime proof/publication and admission guards have Rust fixtures.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = (name) => readFileSync(path.join(root, name), "utf8");
const routes = read("crates/server/src/routes/openwiki.rs");
const recovery = routes.slice(
  routes.indexOf("async fn recover_repository("),
  routes.indexOf("fn record_reconciliation_result("),
);

test("restart and periodic recovery cannot turn publication, new events, backlog or direct main changes into admission", () => {
  assert.ok(recovery.startsWith("async fn recover_repository("));
  assert.match(
    recovery,
    /let Some\(run_id\) = state\.active_run_id else \{[^}]*return Ok\(\(\)\);\s*\};/s,
  );
  for (const forbidden of [
    "start_reconciliation",
    "prepare_run(",
    "sync_events(",
    "pending_events(",
    "Workspace::create",
    "CreateSession",
  ]) {
    assert.ok(
      !recovery.includes(forbidden),
      `Recovery must not admit via ${forbidden}`,
    );
  }
  assert.match(recovery, /bootstrap::recover_locked/);
  assert.match(recovery, /AgentRunRecord::find/);
  assert.match(
    recovery,
    /launch_reserved_coding_agent_execution\(deployment, run_id\)/,
  );
  assert.match(recovery, /finish_run\(/);
});

test("standalone admission is private and requires an explicit UI or checked MCP request", () => {
  assert.doesNotMatch(routes, /fn start_reconciliation\(/);
  assert.match(
    routes,
    /async fn start_reconciliation_request\([^)]*request: SyncRepositoryMemory,/s,
  );
  const callers = [...routes.matchAll(/start_reconciliation_request\(/g)];
  assert.equal(
    callers.length,
    3,
    "Only UI, checked MCP and the private definition",
  );
  assert.match(
    routes,
    /start_reconciliation_request\(&deployment, id, request, None\)/,
  );
  assert.match(
    routes,
    /start_reconciliation_request\(\s*deployment,\s*id,\s*request,\s*Some\(\(expected_branch, expected_source\)\)/s,
  );
  assert.match(
    read("crates/server/src/routes/repo.rs"),
    /memory\/sync", post\(super::openwiki::sync\)/,
  );
  assert.match(
    read("crates/server/src/remote_mcp/repository_wiki.rs"),
    /openwiki::sync_checked\(/,
  );
});

test("no external caller can revive the retired event admission function", () => {
  const walk = (dir) =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const name = path.join(dir, entry.name);
      return entry.isDirectory()
        ? walk(name)
        : name.endsWith(".rs")
          ? [name]
          : [];
    });
  for (const name of walk(path.join(root, "crates"))) {
    assert.doesNotMatch(
      readFileSync(name, "utf8"),
      /\bstart_reconciliation\s*\(/,
      name,
    );
  }
});
