// Only disposable instance data; never accepts an external DB or server URL.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { createInterface } from "node:readline";
import { isolatedEnvironment } from "../server/fixtures/issue-comments-env.mjs";
await mkdir(".build", { recursive: true });
const instance = await mkdtemp(resolve(".build/project-key-instance-"));
const env = isolatedEnvironment(process.env, instance, process.cwd());
const fixtureBin = `${instance}/bin`;
await mkdir(fixtureBin);
for (const name of ["codex", "claude"])
  await writeFile(`${fixtureBin}/${name}`, "#!/bin/sh\nexit 1\n", {
    mode: 0o755,
  });
env.PATH = `${fixtureBin}:${env.PATH}`;
const children = [];
let logs = "";
function start(name, args = [], extra = {}) {
  const child = spawn(resolve(`target/debug/${name}`), args, {
    env: { ...env, ...extra },
    stdio: ["pipe", "pipe", "pipe"],
  });
  const stopped = new Promise((done, reject) => {
    child.once("close", done);
    child.once("error", reject);
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk;
  });
  children.push({ child, stopped });
  return child;
}
let base;
async function api(path, method = "GET", data, expected = 200) {
  const response = await fetch(base + path, {
    method,
    redirect: "error",
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: data === undefined ? undefined : JSON.stringify(data),
  });
  const body = await response.text();
  assert.equal(response.status, expected, `${method} ${path}: ${body}`);
  return expected === 200 ? JSON.parse(body) : body;
}
try {
  const server = start("server");
  server.stdout.on("data", (chunk) => {
    logs += chunk;
  });
  for (let n = 0; n < 120; n++) {
    if (server.exitCode !== null) throw Error("Isolated server exited");
    try {
      const ports = JSON.parse(
        await readFile(`${instance}/ports/vibe-kanban.port`, "utf8"),
      );
      const url = `http://127.0.0.1:${ports.main_port}`;
      if (
        (await fetch(url + "/api/info", { signal: AbortSignal.timeout(1000) }))
          .ok
      ) {
        base = url;
        break;
      }
    } catch {}
    await delay(500);
  }
  assert.ok(base, "Isolated server startup");
  const organization_id = "00000000-0000-0000-0000-000000000002";
  const create = {
    organization_id,
    name: "Lucky Vibe Kanban",
    color: "210 80% 52%",
  };
  await api("/api/local/v1/projects", "POST", create, 422);
  for (const key of ["A", "1AB", "A-B", "ＡＢ", "ABCDEFGHIJK"])
    await api("/api/local/v1/projects", "POST", { ...create, key }, 400);
  const project = (
    await api("/api/local/v1/projects", "POST", { ...create, key: " lvk " })
  ).data;
  assert.equal(project.key, "LVK");
  await api("/api/local/v1/projects", "POST", { ...create, key: "lvk" }, 409);
  const changed = (
    await api(`/api/local/v1/projects/${project.id}`, "PATCH", {
      name: "Renamed",
      key: "OTHER",
    })
  ).data;
  assert.equal(changed.key, "LVK");
  const statuses = (
    await api(
      `/api/local/v1/fallback/project_statuses?project_id=${project.id}`,
    )
  ).project_statuses;
  const issues = await Promise.all(
    Array.from({ length: 12 }, (_, n) =>
      api("/api/local/v1/issues", "POST", {
        project_id: project.id,
        status_id: statuses[0].id,
        title: `Issue ${n}`,
        sort_order: n,
        extension_metadata: null,
      }),
    ),
  );
  assert.deepEqual(
    issues.map((r) => r.data.issue_number).sort((a, b) => a - b),
    Array.from({ length: 12 }, (_, n) => n + 1),
  );
  for (const { data: issue } of issues)
    assert.equal(issue.simple_id, `LVK-${issue.issue_number}`);
  const found = await api("/api/board/issues/search", "POST", {
    project_id: project.id,
    simple_id: "lvk-1",
  });
  assert.equal(found.data.issues[0].simple_id, "LVK-1");
  const empty = (
    await api("/api/local/v1/projects", "POST", { ...create, key: "TYPO" })
  ).data;
  await api(`/api/local/v1/projects/${empty.id}`, "DELETE");
  assert.equal(
    (await api("/api/local/v1/projects", "POST", { ...create, key: "typo" }))
      .data.key,
    "TYPO",
  );

  const mcp = start("vibe-kanban-mcp", ["--mode", "global"], {
    VIBE_BACKEND_URL: base,
  });
  let sequence = 0;
  const pending = new Map();
  createInterface({ input: mcp.stdout }).on("line", (line) => {
    const msg = JSON.parse(line),
      entry = pending.get(msg.id);
    if (entry) {
      pending.delete(msg.id);
      clearTimeout(entry.timer);
      msg.error
        ? entry.reject(Error(JSON.stringify(msg.error)))
        : entry.resolve(msg.result);
    }
  });
  const rpc = (method, params) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(Error(`MCP timeout ${method}`));
      }, 15000);
      pending.set(id, { resolve, reject, timer });
      mcp.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
      );
    });
  await rpc("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "project-key-test", version: "1" },
  });
  mcp.stdin.write(
    JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) +
      "\n",
  );
  const result = await rpc("tools/call", {
    name: "list_issues",
    arguments: { project_id: project.id, simple_id: "lvk-1" },
  });
  assert.ok(!result.isError, JSON.stringify(result));
  const summary =
    result.structuredContent ??
    JSON.parse(result.content.find((c) => c.type === "text").text);
  assert.equal(summary.issues[0].simple_id, "LVK-1");
  console.log(
    "PASS: isolated REST validation, missing key, uniqueness, immutable key, 12 concurrent issues, search, key reuse; real stdio MCP search returns LVK-1.",
  );
} catch (error) {
  await writeFile(".build/project-keys-server.log", logs);
  throw error;
} finally {
  for (const { child, stopped } of children.reverse()) {
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 10000);
    try {
      await stopped;
    } finally {
      clearTimeout(timer);
    }
  }
  await rm(instance, { recursive: true, force: true });
}
