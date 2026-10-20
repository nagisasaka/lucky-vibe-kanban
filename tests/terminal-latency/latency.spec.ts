import { test, expect, type WebSocketRoute } from "@playwright/test";
import { mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

for (const outcome of [
  "completed",
  "failed",
  "cancelled",
  "reconnect",
  "reload",
]) {
  test(`${outcome}: terminal enables next action`, async ({
    page,
    request,
    baseURL,
  }) => {
    const root = process.env.LVK_LATENCY_FIXTURE_ROOT!;
    expect(
      root,
      "Requires a disposable instance configured with codex-fixture.mjs",
    ).toBeTruthy();
    const directory = join(root, randomUUID());
    mkdirSync(directory, { recursive: true });
    const csrf = (await (await request.get("/api/access/status")).json()).csrf;
    const api = async (path: string, data?: unknown) => {
      const response = await request.fetch(path, {
        method: data === undefined ? "GET" : "POST",
        data,
        headers: { Origin: baseURL!, "X-LVK-CSRF": csrf },
      });
      const text = await response.text();
      expect(response.ok(), text).toBeTruthy();
      const body = JSON.parse(text);
      expect(body.success, JSON.stringify(body)).toBeTruthy();
      return body.data;
    };
    const config = { executor: "CODEX", execution_mode: "code" };
    const created = await api("/api/workspaces/start", {
      mode: "direct_folder",
      name: `Terminal latency ${outcome}`,
      repos: [],
      directory_path: directory,
      linked_issue: null,
      executor_config: config,
      attachment_ids: [],
      prompt: "Deterministic latency fixture",
    });
    const runId = created.agent_run.agent_run_id;
    const sessionId = created.agent_run.state.session_id;
    const received: { type: string; at: string }[] = [];
    const routed: { client: WebSocketRoute; server: WebSocketRoute }[] = [];
    if (outcome === "reconnect") {
      await page.routeWebSocket(
        new RegExp(`/api/agent-runs/${runId}/events/ws`),
        (client) => {
          // Forward real backend frames unchanged; inject only a connection close.
          routed.push({ client, server: client.connectToServer() });
        },
      );
    }

    page.on("websocket", (socket) => {
      if (!socket.url().includes(`/agent-runs/${runId}/events/ws`)) return;
      socket.on("framereceived", ({ payload }) => {
        const text = payload.toString();
        if (/succeeded|failed|cancelled/.test(text))
          received.push({ type: "terminal_ws", at: new Date().toISOString() });
      });
    });
    await page.goto(`/workspaces/${created.workspace.id}`);
    await page.keyboard.press("Escape");
    const editor = page.getByRole("textbox", {
      name: "Markdown editor",
      exact: true,
    });
    await expect(editor).toBeVisible({ timeout: 90_000 });
    const queue = page.getByRole("button", {
      name: /^(Queue|キューに追加)( unavailable:.*)?$/,
    });
    await expect(queue).toBeVisible({ timeout: 30_000 });
    const rejected = await request.post(
      `/api/sessions/${sessionId}/follow-up`,
      {
        headers: { Origin: baseURL!, "X-LVK-CSRF": csrf },
        data: {
          prompt: "Must not launch while active",
          executor_config: config,
        },
      },
    );
    expect(rejected.status()).toBe(400);
    expect(await rejected.text()).toContain("already running");
    await editor.fill("Immediate follow-up");
    // Observe the actual DOM transition, independent of Playwright polling.
    await page.evaluate(() => {
      const mark = () => {
        const send = [...document.querySelectorAll("button")].find(
          (button) =>
            /^(Send|送信)$/.test(button.textContent?.trim() ?? "") &&
            !button.disabled,
        );
        if (send && !(window as any).__terminalEnabledAt) {
          (window as any).__terminalEnabledAt = new Date().toISOString();
        }
      };
      new MutationObserver(mark).observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
      });
    });
    if (outcome === "cancelled") {
      await api(`/api/agent-runs/${runId}/cancel`, {
        command_id: randomUUID(),
        idempotency_key: randomUUID(),
        correlation_id: randomUUID(),
        created_at: new Date().toISOString(),
        reason: "Terminal latency cancellation regression",
      });
    } else {
      if (outcome === "reload") await page.goto("about:blank");
      if (outcome === "reconnect") {
        expect(routed.length).toBeGreaterThan(0);
        await routed[0].client.close({
          code: 1012,
          reason: "Regression reconnect",
        });
        await routed[0].server.close();
      }
      writeFileSync(
        join(directory, "release"),
        outcome === "failed" ? "failed" : "completed",
      );
    }
    const expected =
      outcome === "failed"
        ? "failed"
        : outcome === "cancelled"
          ? "cancelled"
          : "succeeded";
    await expect
      .poll(async () => (await api(`/api/agent-runs/${runId}`)).status, {
        timeout: 30_000,
      })
      .toBe(expected);
    const canonicalAt = new Date().toISOString();
    if (outcome === "reconnect") {
      await expect.poll(() => routed.length).toBeGreaterThan(1);
      expect(routed[1].client.url()).toContain("after_sequence=");
    }

    if (outcome === "reload") {
      await page.goto(`/workspaces/${created.workspace.id}`);
      await page.keyboard.press("Escape");
      await expect(editor).toBeVisible();
      await editor.fill("Immediate follow-up");
    }
    const send = page.getByRole("button", {
      name: /^(Send|送信)( unavailable:.*)?$/,
    });
    await expect(send).toBeEnabled({ timeout: 30_000 });
    const enabledAt = await page.evaluate(
      () => (window as any).__terminalEnabledAt ?? new Date().toISOString(),
    );
    // A new run must be admitted by the real ownership/exclusion checks.
    rmSync(join(directory, "release"), { force: true });
    const nextResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        response.url().includes(`/api/sessions/${sessionId}/follow-up`),
    );
    await send.click();
    const next = await nextResponse;
    expect(next.ok(), await next.text()).toBeTruthy();
    const nextRunId = (await next.json()).data.agent_run_id;
    await expect(queue).toBeVisible();
    writeFileSync(join(directory, "release"), "completed");
    await expect
      .poll(
        async () => {
          const runs = await api(`/api/agent-runs/session/${sessionId}`);
          return (
            runs.length === 2 &&
            runs.find((run: any) => run.agent_run_id === nextRunId)?.state
              .status === "succeeded" &&
            runs.find((run: any) => run.agent_run_id === runId)?.state
              .status === expected
          );
        },
        { timeout: 30_000 },
      )
      .toBe(true);
    const provider = JSON.parse(
      readFileSync(join(directory, "observations.jsonl"), "utf8").split(
        "\n",
      )[0],
    );
    expect(provider.parentExecutable).toBe(
      process.env.LVK_EXPECTED_PROCESS_HOST,
    );
    console.log(
      JSON.stringify({
        outcome,
        runId,
        provider,
        received,
        canonicalAt,
        enabledAt,
        providerToEnabledMs: Date.parse(enabledAt) - Date.parse(provider.at),
      }),
    );
  });
}
