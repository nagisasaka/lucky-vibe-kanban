import { expect, test } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { resolve, join } from "node:path";

test("manual archive: UI and MCP confirmation, busy/race/failure, retained history and resume", async ({
  page,
  request,
}, testInfo) => {
  test.skip(
    process.env.LVK_E2E_ARCHIVE !== "1",
    "Requires the isolated archive runner",
  );
  test.setTimeout(240_000);
  const instance = process.env.LVK_INSTANCE_DIR!;
  expect(instance.startsWith(resolve(".build/archive-instance-"))).toBeTruthy();
  expect(
    readFileSync(join(instance, "archive-fixture-owner"), "utf8"),
  ).toContain("LOCAL-105");
  const access = await (await request.get("/api/access/status")).json();
  const headers = { "x-lvk-csrf": access.csrf };
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, { method, data, headers });
    expect(response.ok(), await response.text()).toBeTruthy();
    const body = await response.json();
    expect(body.success, JSON.stringify(body)).toBe(true);
    return body.data;
  };
  const seed = (sql: string, args: unknown[] = []) =>
    execFileSync("python3", [
      "-c",
      `
import sqlite3,sys,json,uuid
from pathlib import Path
root=Path(sys.argv[1]).resolve()
assert root.parent==Path('.build').resolve() and root.name.startswith('archive-instance-')
assert (root/'archive-fixture-owner').read_text().startswith('LOCAL-105')
args=json.loads(sys.argv[3])
args=[uuid.UUID(x['uuid']).bytes if isinstance(x,dict) and 'uuid' in x else x for x in args]
with sqlite3.connect(root/'data/db.v2.sqlite') as db: db.execute(sys.argv[2],args)
`,
      instance,
      sql,
      JSON.stringify(args),
    ]);
  const git = (path: string, ...args: string[]) =>
    execFileSync("git", ["-C", path, ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
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
  const source = join(instance, "fixture-repo");
  mkdirSync(source);
  git(source, "init", "-b", "main");
  git(source, "config", "user.name", "Fixture");
  git(source, "config", "user.email", "fixture@example.invalid");
  writeFileSync(join(source, "retained.txt"), "base\n");
  git(source, "add", ".");
  git(
    source,
    "-c",
    "user.name=Fixture",
    "-c",
    "user.email=fixture@example.invalid",
    "commit",
    "-m",
    "initial",
  );
  const repo = await api("/api/repos", { path: source });
  const create = async (name: string) => {
    const ws = await api("/api/workspaces", { name });
    await api(`/api/workspaces/${ws.id}/repos`, {
      repo_id: repo.id,
      target_branch: "main",
    });
    const session = await api("/api/sessions", {
      workspace_id: ws.id,
      executor: "CODEX",
      name: "Retained history",
    });
    const current = await api(`/api/workspaces/${ws.id}`);
    expect(current.container_ref.startsWith(instance + "/")).toBeTruthy();
    return { ...current, session };
  };
  const mcp = async (args: unknown) => {
    const base = new URL(testInfo.project.use.baseURL!);
    const child = spawn(
      resolve("target/debug/vibe-kanban-mcp"),
      ["--mode", "global"],
      {
        env: { ...process.env, MCP_HOST: base.hostname, MCP_PORT: base.port },
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
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
        clientInfo: { name: "archive-fixture", version: "1" },
      });
      expect(JSON.parse((await replies.next()).value!).error).toBeUndefined();
      child.stdin.write(
        JSON.stringify({
          jsonrpc: "2.0",
          method: "notifications/initialized",
        }) + "\n",
      );
      send(2, "tools/call", { name: "update_workspace", arguments: args });
      const result = JSON.parse((await replies.next()).value!);
      expect(result.error).toBeUndefined();
      return {
        error: result.result.isError === true,
        data: JSON.parse(result.result.content[0].text),
      };
    } finally {
      lines.close();
      child.stdin.end();
      child.kill("SIGTERM");
      await stopped;
    }
  };
  const open = async (ws: any) => {
    await page.goto(`/workspaces/${ws.id}?sessionId=${ws.session.id}`);
    await expect(page.getByTestId("workspace-tokens")).toBeVisible();
  };
  const clickArchive = async () => {
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Archive", exact: true })
      .click({ timeout: 30_000 });
  };
  const waitForIdle = async (ws: any) => {
    // Initial inspection can briefly spawn processes in this fixture's cwd.
    // Success cases wait for their observable use to settle. Deliberate busy
    // cases below do not use this helper and must reject immediately.
    await expect
      .poll(
        async () => {
          const response = await request.post(
            `/api/workspaces/${ws.id}/archive/prepare`,
            { headers },
          );
          const result = await response.json();
          if (!result.success) {
            expect(JSON.stringify(result)).toContain(
              "使用中のWorkspaceはarchiveできません。",
            );
          }
          return result.success;
        },
        { timeout: 15_000 },
      )
      .toBe(true);
  };
  // KeyboardDialog is a portal without an ARIA dialog role.
  const dialog = page.getByRole("heading", {
    name: "ワークツリーが削除されます",
    exact: true,
  });
  const showIdleConfirmation = async (ws: any) => {
    for (let attempt = 0; attempt < 3; attempt++) {
      await waitForIdle(ws);
      await clickArchive();
      await expect(
        dialog.or(page.getByRole("heading", { name: "Error", exact: true })),
      ).toBeVisible();
      if (await dialog.isVisible()) return;
      // A new inspection child can start after the successful preflight.
      // Assert the required immediate refusal, then model an explicit retry.
      // Deliberate busy/race cases never take this retry path.
      const busyError = page.getByText(
        /使用中のWorkspaceはarchiveできません。.*PID \d+:/,
      );
      await expect(busyError).toBeVisible();
      console.log(
        "Fixture inspection race rejected:",
        await busyError.textContent(),
      );
      await expect(dialog).toHaveCount(0);
      expect((await api(`/api/workspaces/${ws.id}`)).archived).toBe(false);
      expect(existsSync(ws.container_ref)).toBe(true);
      await page.getByRole("button", { name: "OK", exact: true }).click();
    }
    throw new Error("Fixture inspection processes did not settle");
  };
  const ws = await create("Archive UI fixture");
  const tree = join(ws.container_ref, repo.name);
  writeFileSync(join(tree, "retained.txt"), "committed work\n");
  git(tree, "add", ".");
  git(
    tree,
    "-c",
    "user.name=Fixture",
    "-c",
    "user.email=fixture@example.invalid",
    "commit",
    "-m",
    "retained work",
  );
  await open(ws);
  await showIdleConfirmation(ws);
  await expect(dialog).toHaveCount(1);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect((await api(`/api/workspaces/${ws.id}`)).archived).toBe(false);
  expect(existsSync(ws.container_ref)).toBe(true);
  await showIdleConfirmation(ws);
  await expect(dialog).toHaveCount(1);
  await page.screenshot({
    path: testInfo.outputPath("archive-confirmation.png"),
  });
  await page
    .getByRole("button", { name: "Archive", exact: true })
    .last()
    .click();
  await expect
    .poll(async () => (await api(`/api/workspaces/${ws.id}`)).worktree_deleted)
    .toBe(true);
  expect((await api(`/api/workspaces/${ws.id}`)).archived).toBe(true);
  expect(existsSync(ws.container_ref)).toBe(false);
  expect(git(source, "show", `${ws.branch}:retained.txt`)).toBe(
    "committed work\n",
  );
  expect(await api(`/api/sessions/${ws.session.id}`)).toBeTruthy();
  await open(ws);
  await page.reload();
  await expect(page.getByTestId("workspace-tokens")).toBeVisible();
  expect(existsSync(ws.container_ref)).toBe(false);
  const diff = await page.evaluate(
    async (id) =>
      new Promise<string>((resolve, reject) => {
        const socket = new WebSocket(
          `${location.origin.replace(/^http/, "ws")}/api/workspaces/${id}/git/diff/ws`,
        );
        const messages: string[] = [];
        const timeout = setTimeout(() => {
          socket.close();
          reject(new Error("diff timeout"));
        }, 10000);
        socket.onmessage = (event) => {
          messages.push(String(event.data));
          if (String(event.data).includes("Ready")) {
            clearTimeout(timeout);
            socket.close();
            resolve(messages.join("\n"));
          }
        };
        socket.onerror = () => {
          clearTimeout(timeout);
          reject(new Error("diff websocket failed"));
        };
      }),
    ws.id,
  );
  expect(diff).toContain("committed work");
  expect(existsSync(ws.container_ref)).toBe(false);
  await api(`/api/workspaces/${ws.id}`, { archived: false }, "PUT");
  expect(existsSync(tree)).toBe(true);
  expect(readFileSync(join(tree, "retained.txt"), "utf8")).toBe(
    "committed work\n",
  );

  // Merge only inside this throwaway repository, never the development repo.
  await api(`/api/workspaces/${ws.id}/git/merge`, { repo_id: repo.id });
  expect((await api(`/api/workspaces/${ws.id}`)).archived).toBe(false);
  expect(existsSync(tree)).toBe(true);

  // UI preflight rejects before displaying a confirmation; MCP uses the same gate.
  const busy = await create("Busy fixture");
  const processId = crypto.randomUUID();
  seed(
    "INSERT INTO execution_processes(id,session_id,executor_action,run_reason,status,started_at,created_at,updated_at) VALUES(?,?,?,'devserver','running',datetime('now'),datetime('now'),datetime('now'))",
    [
      { uuid: processId },
      { uuid: busy.session.id },
      JSON.stringify({
        typ: {
          type: "ScriptRequest",
          script: "fixture-only",
          language: "Bash",
          context: "DevServer",
          working_dir: null,
        },
        next_action: null,
      }),
    ],
  );
  await open(busy);
  await clickArchive();
  await expect(
    page.getByText("使用中のWorkspaceはarchiveできません。", { exact: false }),
  ).toBeVisible();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("button", { name: "OK", exact: true }).click();
  expect((await mcp({ workspace_id: busy.id, archived: true })).error).toBe(
    true,
  );
  seed(
    "UPDATE execution_processes SET status='completed',completed_at=datetime('now') WHERE id=?",
    [{ uuid: processId }],
  );
  const prepared = await mcp({ workspace_id: busy.id, archived: true });
  expect(prepared.error).toBe(false);
  expect(prepared.data.message).toBe("ワークツリーが削除されます");
  expect(existsSync(busy.container_ref)).toBe(true); // MCP cancel = no confirm call.
  seed(
    "UPDATE execution_processes SET status='running',completed_at=NULL WHERE id=?",
    [{ uuid: processId }],
  );
  expect(
    (
      await mcp({
        workspace_id: busy.id,
        archived: true,
        confirmation_token: prepared.data.confirmation_token,
      })
    ).error,
  ).toBe(true);
  expect((await api(`/api/workspaces/${busy.id}`)).archived).toBe(false);
  expect(existsSync(busy.container_ref)).toBe(true);
  seed(
    "UPDATE execution_processes SET status='completed',completed_at=datetime('now') WHERE id=?",
    [{ uuid: processId }],
  );
  const ready = await mcp({ workspace_id: busy.id, archived: true });
  const done = await mcp({
    workspace_id: busy.id,
    archived: true,
    confirmation_token: ready.data.confirmation_token,
  });
  expect(done.error).toBe(false);
  expect(done.data.workspace.worktree_deleted).toBe(true);
  expect(
    (
      await mcp({
        workspace_id: busy.id,
        archived: true,
        confirmation_token: ready.data.confirmation_token,
      })
    ).error,
  ).toBe(true);

  // These are our own foreground fixture children, with no DB/registry entries.
  // They start after prepare to exercise the final OS observation as well.
  for (const reference of ["cwd", "frontend-file"]) {
    const external = await create(`Unregistered ${reference} fixture`);
    const assets = join(
      external.container_ref,
      repo.name,
      "packages/local-web/dist",
    );
    mkdirSync(assets, { recursive: true });
    const asset = join(assets, "index.html");
    writeFileSync(asset, "LOCAL-105 frontend fixture");
    const ready = await mcp({ workspace_id: external.id, archived: true });
    expect(ready.error).toBe(false);
    const child = spawn(
      "python3",
      [
        "-c",
        [
          "import sys,os,time",
          "target=sys.stdin.readline().strip()",
          reference === "cwd" ? "os.chdir(target)" : "asset=open(target)",
          "print('ready',flush=True)",
          "time.sleep(60)",
        ].join("\n"),
      ],
      { cwd: instance, stdio: ["pipe", "pipe", "pipe"] },
    );
    child.stderr.resume();
    const stopped = new Promise<void>((done) =>
      child.once("close", () => done()),
    );
    const lines = createInterface({ input: child.stdout });
    try {
      child.stdin.end(
        (reference === "cwd" ? external.container_ref : asset) + "\n",
      );
      expect((await lines[Symbol.asyncIterator]().next()).value).toBe("ready");
      await open(external);
      await clickArchive();
      await expect(
        page.getByText("使用中のWorkspaceはarchiveできません。", {
          exact: false,
        }),
      ).toBeVisible();
      await expect(dialog).toHaveCount(0);
      await page.screenshot({
        path: testInfo.outputPath(`unregistered-${reference}-rejected.png`),
      });
      await page.getByRole("button", { name: "OK", exact: true }).click();
      expect(
        (await mcp({ workspace_id: external.id, archived: true })).error,
      ).toBe(true);
      expect(
        (
          await mcp({
            workspace_id: external.id,
            archived: true,
            confirmation_token: ready.data.confirmation_token,
          })
        ).error,
      ).toBe(true);
      expect((await api(`/api/workspaces/${external.id}`)).archived).toBe(
        false,
      );
      expect(existsSync(external.container_ref)).toBe(true);
    } finally {
      lines.close();
      child.kill("SIGTERM");
      await stopped;
    }
    const retry = await mcp({ workspace_id: external.id, archived: true });
    expect(retry.error).toBe(false);
    expect(
      (
        await mcp({
          workspace_id: external.id,
          archived: true,
          confirmation_token: retry.data.confirmation_token,
        })
      ).error,
    ).toBe(false);
    expect(existsSync(external.container_ref)).toBe(false);
  }

  // Archive does not grant execution-only workspaces permission to resume work.
  // Exercise both the unreleased label and the state after owner release.
  for (const usage of ["execution_only", "interactive"]) {
    const completed = await create(`Completed ${usage} fixture`);
    seed("UPDATE workspaces SET usage=? WHERE id=?", [
      usage,
      { uuid: completed.id },
    ]);
    if (usage === "execution_only") {
      const denied = await request.put(`/api/workspaces/${completed.id}`, {
        headers,
        data: { name: "must remain denied" },
      });
      expect(denied.status()).toBe(409);
    }
    await open(completed);
    await showIdleConfirmation(completed);
    await expect(dialog).toHaveCount(1);
    await page.screenshot({
      path: testInfo.outputPath(`completed-${usage}-confirmation.png`),
    });
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    const ready = await mcp({ workspace_id: completed.id, archived: true });
    expect(ready.error).toBe(false);
    const done = await mcp({
      workspace_id: completed.id,
      archived: true,
      confirmation_token: ready.data.confirmation_token,
    });
    expect(done.error).toBe(false);
    expect(done.data.workspace.archived).toBe(true);
    expect(done.data.workspace.worktree_deleted).toBe(true);
    expect(done.data.workspace.usage).toBe(usage);
    expect(existsSync(completed.container_ref)).toBe(false);
  }

  const empty = await api("/api/workspaces", { name: "Empty fixture" });
  const emptyReady = await mcp({ workspace_id: empty.id, archived: true });
  expect(emptyReady.error).toBe(false);
  expect(
    (
      await mcp({
        workspace_id: empty.id,
        archived: true,
        confirmation_token: emptyReady.data.confirmation_token,
      })
    ).error,
  ).toBe(false);
  expect(
    (await api(`/api/workspaces/${empty.id}`, { archived: false }, "PUT"))
      .archived,
  ).toBe(false);

  const failure = await create("Deletion failure fixture");
  const invalid = join(instance, "not-a-directory");
  writeFileSync(invalid, "retained");
  seed("UPDATE workspaces SET container_ref=? WHERE id=?", [
    invalid,
    { uuid: failure.id },
  ]);
  await open(failure);
  await clickArchive();
  await expect(dialog).toHaveCount(1);
  await page
    .getByRole("button", { name: "Archive", exact: true })
    .last()
    .click();
  await expect(
    page.getByText(
      /ワークツリーの物理削除に失敗しました（.+）。手動で削除してください。/,
    ),
  ).toBeVisible();
  const failed = await api(`/api/workspaces/${failure.id}`);
  expect(failed.archived).toBe(true);
  expect(failed.worktree_deleted).toBe(false);
  expect(existsSync(invalid)).toBe(true);
  await page.screenshot({
    path: testInfo.outputPath("archive-deletion-failure.png"),
  });
  expect(
    readFileSync(
      join(instance, "runtime/worktrees/orphan-fixture/retained"),
      "utf8",
    ),
  ).toBe("retain\n");
});
