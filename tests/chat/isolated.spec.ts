import {
  expect,
  test,
  type APIRequestContext,
  type Page,
  type WebSocketRoute,
} from "@playwright/test";
import { randomUUID } from "node:crypto";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

async function api(
  request: APIRequestContext,
  path: string,
  data?: unknown,
  method = data === undefined ? "GET" : "POST",
) {
  const response = await request.fetch(path, {
    method,
    data,
    headers: { "x-lvk-csrf": "local", origin: process.env.LVK_E2E_BASE_URL! },
  });
  expect(response.ok(), `${path}: ${await response.text()}`).toBeTruthy();
  const body = await response.json();
  expect(body.success, JSON.stringify(body)).toBeTruthy();
  return body.data;
}
async function configure(request: APIRequestContext) {
  const info = await api(request, "/api/info");
  await api(
    request,
    "/api/config",
    {
      ...info.config,
      disclaimer_acknowledged: true,
      onboarding_acknowledged: true,
      remote_onboarding_acknowledged: true,
      analytics_enabled: false,
      showcases: { seen_features: ["workspaces-guide"] },
      language: "EN",
      send_message_shortcut: "Enter",
    },
    "PUT",
  );
}
async function workspace(request: APIRequestContext, name: string) {
  const workspace = await api(request, "/api/workspaces", { name });
  const session = await api(request, "/api/sessions", {
    workspace_id: workspace.id,
    executor: "CODEX",
    name: "Chat regression",
  });
  return { workspace, session };
}
function run(session: string, index: number, status = "succeeded") {
  const id = randomUUID(),
    attempt = randomUUID();
  const state = {
    state_schema_version: 1,
    reducer_version: 1,
    session_id: session,
    agent_run_id: id,
    turn_id: randomUUID(),
    status,
    projection_status: "current",
    last_run_attempt_id: attempt,
    last_run_attempt_number: 1,
    last_event_sequence: 0,
    last_event_id: null,
    provider_session: null,
    terminal_output: null,
    last_error: null,
    unknown_event_count: 0,
    updated_at: new Date(2026, 0, index + 1).toISOString(),
  };
  return {
    agent_run_id: id,
    session_id: session,
    turn_id: state.turn_id,
    state,
    created_at: state.updated_at,
    updated_at: state.updated_at,
  };
}
type SyntheticRun = ReturnType<typeof run>;

function message(
  run: SyntheticRun,
  sequence: number,
  content: string,
  role = "user",
  native = false,
) {
  return {
    schema_version: 1,
    payload_version: 1,
    event_id: `${run.agent_run_id}-${sequence}`,
    session_id: run.session_id,
    agent_run_id: run.agent_run_id,
    turn_id: run.turn_id,
    run_attempt_id: run.state.last_run_attempt_id,
    run_attempt_number: 1,
    sequence,
    correlation_id: randomUUID(),
    timestamp: new Date(2026, 1, 1, 0, 0, sequence).toISOString(),
    native_refs: native
      ? [
          {
            sequence: sequence * 100,
            run_attempt_id: run.state.last_run_attempt_id,
          },
        ]
      : [],
    payload: {
      type: "message",
      data: {
        message: { message_id: `message-${sequence}`, role, content },
        final_output: false,
      },
    },
  };
}
async function scroller(page: Page) {
  return page
    .locator(".overflow-y-auto")
    .filter({ has: page.locator("[data-semantic-key]") })
    .last();
}
async function anchor(page: Page) {
  return (await scroller(page)).evaluate((el) => {
    const top = el.getBoundingClientRect().top;
    const row = [
      ...el.querySelectorAll<HTMLElement>("[data-semantic-key]"),
    ].find((row) => row.getBoundingClientRect().bottom > top)!;
    return {
      key: row.dataset.semanticKey!,
      top: row.getBoundingClientRect().top - top,
    };
  });
}
async function anchorTop(page: Page, key: string) {
  return (await scroller(page)).evaluate((el, key) => {
    const row = [
      ...el.querySelectorAll<HTMLElement>("[data-semantic-key]"),
    ].find((row) => row.dataset.semanticKey === key);
    return row
      ? row.getBoundingClientRect().top - el.getBoundingClientRect().top
      : null;
  }, key);
}

test("built application: recent-two loading, prepend, following, reading and delayed workspace isolation (synthetic events)", async ({
  page,
  request,
}) => {
  await configure(request);
  const a = await workspace(request, "LOCAL21 history A");
  const b = await workspace(request, "LOCAL21 history B");
  const aRuns = Array.from({ length: 7 }, (_, i) => run(a.session.id, i));
  const bRuns = [run(b.session.id, 0)];
  const streams = new Map<string, WebSocketRoute>();
  const emit = (r: ReturnType<typeof run>, label: string, ready = true) => {
    const ws = streams.get(r.agent_run_id)!;
    for (let i = 1; i <= 14; i++)
      ws.send(
        JSON.stringify({
          type: "event",
          data: {
            event: message(
              r,
              i,
              `${label}-${i}\n\n${"Message body line\n".repeat(5)}`,
            ),
            replay: true,
          },
        }),
      );
    if (ready)
      ws.send(
        JSON.stringify({
          type: "ready",
          data: {
            state: r.state,
            cursor: { run_attempt_number: 1, sequence: 14 },
          },
        }),
      );
  };
  await page.route("**/api/agent-runs/session/*", (route) =>
    route.fulfill({
      json: {
        success: true,
        data: route.request().url().endsWith(a.session.id) ? aRuns : bRuns,
      },
    }),
  );
  await page.routeWebSocket("**/api/agent-runs/*/events/ws*", (ws) => {
    const id = new URL(ws.url()).pathname.split("/")[3];
    streams.set(id, ws);
    if (id === bRuns[0].agent_run_id) emit(bRuns[0], "OTHER-WORKSPACE");
  });
  await page.goto(`/workspaces/${a.workspace.id}`);
  await page.keyboard.press("Escape");
  await expect.poll(() => streams.size).toBe(2);
  expect([...streams.keys()].sort()).toEqual(
    aRuns
      .slice(-2)
      .map((r) => r.agent_run_id)
      .sort(),
  );
  emit(aRuns[5], "RECENT-5");
  emit(aRuns[6], "RECENT-6");
  await expect(
    page.getByText("RECENT-6-14", { exact: false }).first(),
  ).toBeVisible();
  await expect.poll(() => streams.size).toBe(5);
  const scroll = await scroller(page);
  await expect
    .poll(() =>
      scroll.evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop),
    )
    .toBeLessThan(3);
  await scroll.evaluate((el) => {
    el.dispatchEvent(new WheelEvent("wheel", { deltaY: -500, bubbles: true }));
    el.scrollTop -= 450;
  });
  await page.waitForTimeout(100);
  const position = await anchor(page);
  for (const i of [4, 3, 2]) emit(aRuns[i], `HISTORY-${i}`);
  await expect.poll(() => streams.size).toBe(7);
  await page.waitForTimeout(200);
  expect(
    Math.abs((await anchorTop(page, position.key))! - position.top),
  ).toBeLessThan(3);
  for (const i of [1, 0]) emit(aRuns[i], `HISTORY-${i}`);
  await page.waitForTimeout(200);
  expect(
    Math.abs((await anchorTop(page, position.key))! - position.top),
  ).toBeLessThan(3);
  streams.get(aRuns[6].agent_run_id)!.send(
    JSON.stringify({
      type: "event",
      data: {
        event: message(
          aRuns[6],
          15,
          "NEW RESPONSE WHILE READING\n".repeat(20),
          "assistant",
        ),
        replay: false,
      },
    }),
  );
  await page.waitForTimeout(200);
  expect(
    Math.abs((await anchorTop(page, position.key))! - position.top),
  ).toBeLessThan(3);
  await scroll.evaluate((el) => {
    el.dispatchEvent(new WheelEvent("wheel", { deltaY: 99999, bubbles: true }));
    el.scrollTop = el.scrollHeight;
  });
  await page.waitForTimeout(100);
  streams.get(aRuns[6].agent_run_id)!.send(
    JSON.stringify({
      type: "event",
      data: {
        event: message(aRuns[6], 16, "LATEST FOLLOW\n".repeat(25), "assistant"),
        replay: false,
      },
    }),
  );
  await expect
    .poll(() =>
      scroll.evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop),
    )
    .toBeLessThan(3);
  await page.screenshot({ path: test.info().outputPath("following.png") });
  await page.getByText("LOCAL21 history B", { exact: true }).first().click();
  await expect(
    page.getByText("OTHER-WORKSPACE-14", { exact: false }).first(),
  ).toBeVisible();
  // Deliver a callback from the old transport after navigation if still open.
  try {
    emit(aRuns[6], "STALE-OLD-WORKSPACE");
  } catch {
    /* closed transport is also correct */
  }
  await page.waitForTimeout(200);
  await expect(
    page.getByText("STALE-OLD-WORKSPACE", { exact: false }),
  ).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath("workspace-b.png") });
});

test("built application: newline send, failure, later input, Enter/Ctrl+Enter and IME (mock send, real Scratch)", async ({
  page,
  request,
}) => {
  await configure(request);
  const { workspace: w, session } = await workspace(
    request,
    "LOCAL21 composer",
  );
  let release: (() => void) | undefined;
  let fail = false;
  let requests = 0;
  const submittedPrompts: string[] = [];
  await page.route(`**/api/sessions/${session.id}/follow-up`, async (route) => {
    requests++;
    submittedPrompts.push(route.request().postDataJSON().prompt);
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    await route.fulfill({
      status: fail ? 500 : 200,
      json: {
        success: !fail,
        data: {},
        message: fail ? "Injected send failure" : null,
      },
    });
  });
  await page.goto(`/workspaces/${w.id}`);
  await page.keyboard.press("Escape");
  const editor = page
    .locator('[contenteditable="true"][role="textbox"]')
    .first();
  await expect(editor).toBeVisible();
  await editor.fill("Newline submission");
  await editor.press("Shift+Enter");
  await editor.press("Enter");
  await expect.poll(() => requests).toBe(1);
  release!();
  await expect(editor).toHaveText("");
  await editor.fill("Submitted snapshot");
  await editor.press("Enter");
  await expect.poll(() => requests).toBe(2);
  await editor.fill("Newer input must survive");
  release!();
  await expect(editor).toHaveText("Newer input must survive");
  await page.waitForTimeout(600);
  await expect(editor).toHaveText("Newer input must survive");
  fail = true;
  await editor.press("Enter");
  await expect.poll(() => requests).toBe(3);
  release!();
  await expect(
    page.getByText("Injected send failure", { exact: false }).first(),
  ).toBeVisible();
  await expect(editor).toHaveText("Newer input must survive");
  await editor.dispatchEvent("keydown", {
    key: "Enter",
    code: "Enter",
    isComposing: true,
    keyCode: 229,
  });
  await page.waitForTimeout(100);
  expect(requests).toBe(3);
  const info = await api(request, "/api/info");
  await api(
    request,
    "/api/config",
    { ...info.config, send_message_shortcut: "ModifierEnter" },
    "PUT",
  );
  await page.reload();
  await expect(editor).toHaveText("Newer input must survive");
  fail = false;
  await editor.press("Control+Enter");
  await page.keyboard.insertText("continued line");
  expect(requests).toBe(3);
  await editor.press("Enter");
  await expect.poll(() => requests).toBe(4);
  expect(submittedPrompts.at(-1)).toBe(
    "Newer input must survive\ncontinued line",
  );
  release!();
  await expect(editor).toHaveText("");
  await page.screenshot({ path: test.info().outputPath("composer-clear.png") });
});

test("built application: real Codex steering is persisted once and survives reconnect/reload", async ({
  page,
  request,
}) => {
  test.setTimeout(300_000);
  await configure(request);
  const directory = join(
    process.env.LVK_E2E_INSTANCE!,
    `codex-${randomUUID()}`,
  );
  mkdirSync(directory);
  let runId: string | undefined;
  const connections: Array<{ client: WebSocketRoute; server: WebSocketRoute }> =
    [];
  // Transparent proxy: all messages still come from the real backend. Closing
  // the socket deliberately exercises cursor-based reconnect, not just reload.
  await page.routeWebSocket("**/api/agent-runs/*/events/ws*", (client) => {
    connections.push({ client, server: client.connectToServer() });
  });
  try {
    const created = await api(request, "/api/workspaces/start", {
      mode: "direct_folder",
      name: "LOCAL21 real steering",
      repos: [],
      directory_path: directory,
      linked_issue: null,
      executor_config: { executor: "CODEX", execution_mode: "code" },
      attachment_ids: [],
      prompt:
        "Bounded UI regression test. Do not read or change files, access credentials, use subagents or send network requests. Run a shell command that sleeps for 40 seconds, then reply INITIAL_DONE. If steered, include the supplied token in your final reply. Do nothing else.",
    });
    runId = created.agent_run.agent_run_id;
    await page.goto(`/workspaces/${created.workspace.id}`);
    await page.keyboard.press("Escape");
    const editor = page
      .locator('[contenteditable="true"][role="textbox"]')
      .first();
    const steer = page.getByRole("button", { name: "Steer", exact: true });
    await expect(steer).toBeVisible({ timeout: 120_000 });
    await expect
      .poll(
        async () =>
          (await api(request, `/api/agent-runs/${runId}/events`)).events.some(
            (e: any) =>
              e.payload.type === "tool_call" &&
              ["created", "running"].includes(e.payload.data.status),
          ),
        { timeout: 120_000 },
      )
      .toBe(true);
    const token = `STEER_LOCAL21_${randomUUID()}`;
    const instruction = `Include ${token} in your final response. Do not perform additional work.`;
    await editor.fill(instruction);
    const response = page.waitForResponse(
      (r) =>
        r.request().method() === "POST" &&
        new URL(r.url()).pathname.endsWith("/steer"),
    );
    await steer.click();
    const steerResponse = await response;
    expect(steerResponse.ok()).toBeTruthy();
    const sentInstruction = steerResponse.request().postDataJSON().content;
    await expect(editor).toHaveText("");
    const userRows = page
      .locator("[data-semantic-key]")
      .filter({ hasText: instruction });
    await expect(userRows).toHaveCount(1);
    const previousConnections = connections.length;
    const active = connections.at(-1)!;
    active.client.close({ code: 1012, reason: "isolated reconnect test" });
    active.server.close();
    await expect
      .poll(() => connections.length)
      .toBeGreaterThan(previousConnections);
    await expect(userRows).toHaveCount(1);
    await page.reload();
    await expect(userRows).toHaveCount(1);
    await expect
      .poll(
        async () => (await api(request, `/api/agent-runs/${runId}`)).status,
        { timeout: 150_000 },
      )
      .toBe("succeeded");
    expect(
      (await api(request, `/api/agent-runs/${runId}`)).terminal_output.content,
    ).toContain(token);
    const history = await api(request, `/api/agent-runs/${runId}/events`);
    const inputs = history.events.filter(
      (e: any) =>
        e.payload.type === "message" &&
        e.payload.data.message.role === "user" &&
        e.payload.data.message.content === sentInstruction,
    );
    expect(inputs).toHaveLength(1);
    expect(inputs[0].agent_run_id).toBe(runId);
    expect(inputs[0].session_id).toBe(created.agent_run.state.session_id);
    await page.screenshot({
      path: test.info().outputPath("real-steering.png"),
    });
    console.log(
      "REAL_CODEX_EVIDENCE",
      JSON.stringify({
        workspace: created.workspace.id,
        session: created.agent_run.state.session_id,
        run: runId,
        event: inputs[0].event_id,
      }),
    );
  } finally {
    await page.context().setOffline(false);
    if (runId) {
      const state = await api(request, `/api/agent-runs/${runId}`);
      if (
        ![
          "succeeded",
          "failed",
          "cancelled",
          "crashed",
          "audit_failed",
        ].includes(state.status)
      ) {
        await api(request, `/api/agent-runs/${runId}/cancel`, {
          command_id: randomUUID(),
          idempotency_key: randomUUID(),
          correlation_id: randomUUID(),
          created_at: new Date().toISOString(),
          reason: "LOCAL21 isolated E2E cleanup",
        });
        await expect
          .poll(
            async () => (await api(request, `/api/agent-runs/${runId}`)).status,
            { timeout: 30_000 },
          )
          .toMatch(/succeeded|failed|cancelled|crashed|audit_failed/);
      }
    }
  }
});
