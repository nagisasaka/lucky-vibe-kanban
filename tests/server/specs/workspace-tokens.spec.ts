import { expect, test } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { resolve } from "node:path";

// Explicit isolated-instance guard: never seed a running developer/stable DB.
test("Workspace tokens: unknown, partial, refresh, MCP and restart", async ({
  page,
  request,
}, testInfo) => {
  test.skip(
    process.env.LVK_E2E_TOKENS !== "1",
    "Requires the task-owned isolated runner",
  );
  const instance = process.env.LVK_INSTANCE_DIR!;
  expect(instance.startsWith(resolve(".build/tokens-instance-"))).toBeTruthy();
  const access = await (await request.get("/api/access/status")).json();
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, {
      method,
      data,
      headers: { "x-lvk-csrf": access.csrf },
    });
    expect(response.ok(), await response.text()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    return body.data;
  };
  const mcp = async (workspace: string) => {
    const base = new URL(testInfo.project.use.baseURL!);
    const child = spawn(
      resolve("target/debug/vibe-kanban-mcp"),
      ["--mode", "global"],
      {
        env: { ...process.env, MCP_HOST: base.hostname, MCP_PORT: base.port },
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    // Drain diagnostics; tokens/credentials must not be copied to evidence.
    child.stderr.resume();
    const stopped = new Promise<void>((done) =>
      child.once("close", () => done()),
    );
    const lines = createInterface({ input: child.stdout });
    const replies = lines[Symbol.asyncIterator]();
    const send = (id: number, method: string, params: unknown) =>
      child.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
      );
    try {
      send(1, "initialize", {
        protocolVersion: "2025-03-26",
        capabilities: {},
        clientInfo: { name: "tokens-fixture", version: "1" },
      });
      expect(JSON.parse((await replies.next()).value!).error).toBeUndefined();
      child.stdin.write(
        JSON.stringify({
          jsonrpc: "2.0",
          method: "notifications/initialized",
        }) + "\n",
      );
      send(2, "tools/call", {
        name: "get_workspace_tokens",
        arguments: { workspace_id: workspace },
      });
      const result = JSON.parse((await replies.next()).value!);
      expect(result.error).toBeUndefined();
      expect(result.result.isError).not.toBe(true);
      return JSON.parse(result.result.content[0].text);
    } finally {
      lines.close();
      child.stdin.end();
      child.kill("SIGTERM");
      await stopped;
    }
  };
  const seed = (workspace: string, stage: number) => {
    // Seed only the provider-reported persistence rows. Real ingestion/replay is
    // exercised by Rust fixture tests; no provider run is started by this E2E.
    execFileSync("python3", [
      "-c",
      `
import sqlite3,sys,uuid
from pathlib import Path
root=Path(sys.argv[1]).resolve()
assert root.parent==Path('.build').resolve() and root.name.startswith('tokens-instance-')
db=sqlite3.connect(root/'data/db.v2.sqlite')
w=uuid.UUID(sys.argv[2]).bytes
stage=int(sys.argv[3])
with db:
 db.execute("INSERT INTO workspace_token_usage VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(workspace_id,provider_id,provider_session_id) DO UPDATE SET total_tokens=excluded.total_tokens,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,cached_input_tokens=excluded.cached_input_tokens,reasoning_tokens=excluded.reasoning_tokens,updated_at=excluded.updated_at",(w,'codex','fixture-thread-a',120 if stage==1 else 180,100 if stage==1 else 150,20 if stage==1 else 30,60,10,'2026-10-21T18:00:00Z','2026-10-21T18:01:00Z'))
 if stage==2: db.execute("INSERT OR IGNORE INTO workspace_token_usage VALUES(?,?,?,?,?,?,?,?,?,?)",(w,'codex','fixture-thread-b',20,15,5,None,None,'2026-10-21T18:01:00Z','2026-10-21T18:02:00Z'))
`,
      instance,
      workspace,
      String(stage),
    ]);
  };
  let scope: any;
  if (process.env.LVK_E2E_TOKENS_RESTART === "1") {
    scope = JSON.parse(readFileSync(instance + "/tokens-fixture.json", "utf8"));
  } else {
    const info = await api("/api/info");
    await api(
      "/api/config",
      {
        ...info.config,
        onboarding_acknowledged: true,
        disclaimer_acknowledged: true,
        language: "EN",
        show_release_notes: false,
        showcases: { seen_features: ["workspaces-guide"] },
      },
      "PUT",
    );
    const workspace = await api("/api/workspaces", {
      name: "Workspace token fixture",
    });
    const session = await api("/api/sessions", {
      workspace_id: workspace.id,
      executor: "CODEX",
      name: "Usage fixture",
    });
    scope = { workspace, session };
    writeFileSync(instance + "/tokens-fixture.json", JSON.stringify(scope));
  }
  const displayInfo = await api("/api/info");
  await api("/api/config", { ...displayInfo.config, language: "EN" }, "PUT");
  await page.goto(
    `/workspaces/${scope.workspace.id}?sessionId=${scope.session.id}`,
  );
  const summary = page.getByTestId("workspace-tokens");
  if (process.env.LVK_E2E_TOKENS_RESTART !== "1") {
    await expect(summary).toContainText("Unknown");
    expect(await mcp(scope.workspace.id)).toEqual(
      await api(`/api/workspaces/${scope.workspace.id}/tokens`),
    );
    seed(scope.workspace.id, 1);
    await expect(summary.locator("summary")).toContainText("120");
    await expect(summary).toContainText("Partial");
    seed(scope.workspace.id, 2);
  }
  await expect(summary.locator("summary")).toContainText("200");
  await summary.locator("summary").click();
  await expect(summary).toContainText(
    "Reported cumulative totals of Codex parent threads",
  );
  await expect(summary).toContainText(
    "Child-thread usage not included in the parent is unknown",
  );
  await expect(summary).toContainText("Input: 165");
  await expect(summary).toContainText("Output: 35");
  await expect(summary).toContainText("Cached input (included in input): 60");
  await expect(summary).toContainText("Reasoning (included in output): 10");
  await expect(summary).toContainText("Threads reporting breakdown: 1 / 2");
  await page.screenshot({
    path: testInfo.outputPath("workspace-tokens-details.png"),
  });
  const value = await api(`/api/workspaces/${scope.workspace.id}/tokens`);
  expect(await mcp(scope.workspace.id)).toEqual(value);
  expect(value).not.toHaveProperty("observed_attempt_count");
  expect(value).not.toHaveProperty("attempt_count");
  expect(value.total_tokens).toBe(200);
  expect(value.completeness).toBe("partial");
  await page.reload();
  await expect(
    page.getByTestId("workspace-tokens").locator("summary"),
  ).toContainText("200");
  // Verify the user-facing Japanese range label as well as English details.
  const currentInfo = await api("/api/info");
  await api("/api/config", { ...currentInfo.config, language: "JA" }, "PUT");
  await page.reload();
  await expect(page.getByTestId("workspace-tokens")).toContainText(
    "Workspace累積トークン",
  );
  await expect(page.getByTestId("workspace-tokens")).toContainText("一部のみ");
  await summary.locator("summary").click();
  await expect(summary).toContainText("Codex親threadの報告済み累積値");
  await expect(summary).toContainText("親に含まれない子thread分");
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByTestId("workspace-tokens")).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("workspace-tokens-mobile.png"),
  });
});
