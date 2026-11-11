// Public npm MCP contract: bounded begin views and durable focused-update debt.
// No models, private imports or metadata writes. All repository changes are fixtures.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { openWikiMcp } from "./lib/openwiki-mcp.mjs";

const root = await mkdtemp(path.join(tmpdir(), "lvk-openwiki-focused-"));
const env = {
  ...process.env,
  HOME: path.join(root, "home"),
  OPENWIKI_CONFIG_DIR: path.join(root, "config"),
  OPENWIKI_TELEMETRY_DISABLED: "1",
};
const git = (...args) => {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout.trim();
};
let client;
try {
  git("init", "-b", "main");
  git("config", "user.name", "OpenWiki fixture");
  git("config", "user.email", "fixture@example.invalid");
  await writeFile(path.join(root, ".gitignore"), "home/\nconfig/\n");
  const claims = Array.from({ length: 51 }, (_, i) => ({
    statement: `Limit ${i} permits at most ${i + 1} concurrent jobs.`,
    evidence: [{ resource: `repo://limits.js#L${i + 1}` }],
  }));
  await writeFile(
    path.join(root, "limits.js"),
    claims.map((_, i) => `export const limit${i} = ${i + 1};`).join("\n") +
      "\n",
  );
  git("add", ".");
  git("commit", "-m", "source baseline");
  const sourceBaseline = git("rev-parse", "HEAD");
  const binary = process.env.OPENWIKI_BIN ?? "openwiki";
  client = await openWikiMcp(binary, root, env);
  const tool = (...args) => client.tool(...args);
  const initial = await tool("openwiki_begin", {
    root,
    mode: "init",
    language: "en",
  });
  await tool("openwiki_submit_plan", {
    runId: initial.runId,
    pages: [
      {
        path: "/openwiki/quickstart.md",
        title: "Limits",
        purpose: "Job concurrency limits",
        seedPaths: ["limits.js"],
      },
    ],
  });
  const pending = await tool("openwiki_next_page", { runId: initial.runId });
  await writeFile(
    path.join(root, "openwiki/quickstart.md"),
    "---\ntype: concept\ntitle: Limits\ndescription: Job concurrency limits.\ntags: [limits]\n---\n\n# Limits\n\n" +
      claims.map(({ statement }) => statement).join("\n\n") +
      "\n",
  );
  await tool("openwiki_submit_page", {
    runId: initial.runId,
    jobId: pending.job.id,
    claims,
  });
  assert.equal(
    (await tool("openwiki_finish", { runId: initial.runId })).status,
    "complete",
  );
  git("add", ".");
  git("commit", "-m", "generated fixture baseline");
  // Changed evidence makes all 51 Claims stale; 201 extra paths exceed the host cap.
  await writeFile(
    path.join(root, "limits.js"),
    claims.map((_, i) => `export const limit${i} = ${i + 2};`).join("\n") +
      "\n",
  );
  await mkdir(path.join(root, "src"));
  for (let i = 0; i < 201; i++)
    await writeFile(
      path.join(root, "src", `file-${i}.js`),
      `export const value = ${i};\n`,
    );
  git("add", ".");
  git("commit", "-m", "focused source change");
  const changedHead = git("rev-parse", "HEAD");
  const expectedPaths = git("diff", "--name-only", sourceBaseline, changedHead)
    .split("\n")
    .filter((p) => !p.startsWith("openwiki/") && p !== ".openwikiignore");
  const begin = await tool("openwiki_begin", {
    root,
    mode: "update",
    language: "en",
  });
  assert.equal(begin.status, "active");
  assert.equal(begin.changedPaths.length, 200);
  assert.equal(begin.changedPathsTruncated, true);
  assert.equal(begin.changedPathCount, expectedPaths.length);
  assert.equal(begin.claimIssues.length, 50);
  assert.equal(begin.claimIssueCount, 51);
  assert.equal(begin.claimIssuesTruncated, true);
  assert.ok(begin.claimIssuePages.includes("/openwiki/quickstart.md"));
  assert.ok(
    begin.pageUpdateWindows.some(
      (w) =>
        w.changedPathsTruncated && w.changedPathCount === expectedPaths.length,
    ),
  );
  assert.ok(
    begin.pageUpdateWindows.some((w) =>
      w.commits.some((c) => JSON.stringify(c).includes(changedHead)),
    ),
  );
  const plan = {
    runId: begin.runId,
    pages: [],
    deferUnplannedClaimIssues: true,
  };
  assert.equal((await tool("openwiki_submit_plan", plan)).totalPages, 0);
  await client.close();
  client = await openWikiMcp(binary, root, env);
  const resumed = await tool("openwiki_begin", {
    root,
    mode: "update",
    language: "en",
  });
  assert.equal(resumed.runId, begin.runId);
  assert.equal(resumed.resumed, true);
  assert.equal((await tool("openwiki_submit_plan", plan)).totalPages, 0);
  assert.equal(
    (await tool("openwiki_next_page", { runId: begin.runId })).status,
    "complete",
  );
  const done = await tool("openwiki_finish", { runId: begin.runId });
  assert.equal(done.status, "complete");
  assert.notEqual(done.sourceChanged, true);
  // A completed focused run is not proof that every stale Claim was reconciled.
  const debt = await tool("openwiki_begin", {
    root,
    mode: "update",
    language: "en",
  });
  assert.equal(debt.status, "active");
  assert.equal(debt.claimIssueCount, 51);
  assert.equal(
    (await tool("openwiki_submit_plan", { runId: debt.runId, pages: [] }))
      .totalPages,
    1,
  );
  const job = await tool("openwiki_next_page", { runId: debt.runId });
  assert.equal(job.status, "pending");
  const unfinished = await client.call("tools/call", {
    name: "openwiki_finish",
    arguments: { runId: debt.runId },
  });
  assert.equal(unfinished.isError, true);
  console.log(
    "PASS: public begin >200 paths/51 Claims bounded to 200/50, exact totals and commit context; focused deferral survives restart and finish, debt reappears and default plan requires reconciliation; unfinished writer rejected.",
  );
} finally {
  if (client) await client.close();
  await rm(root, { recursive: true, force: true });
}
