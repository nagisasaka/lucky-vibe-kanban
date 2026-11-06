import { expect, test, type WebSocketRoute } from "@playwright/test";

test("recent conversation displays before older replay; background batches and session isolation", async ({
  page,
}) => {
  const streams = new Map<string, WebSocketRoute>();
  const closed = new Set<string>();
  let opened = 0;
  const state = (id: string) => ({
    agent_run_id: id,
    status: "succeeded",
    projection_status: "current",
    last_run_attempt_number: 1,
    last_event_sequence: 1,
    updated_at: "2026-11-06T00:00:00Z",
    pending_approvals: [],
    pending_inputs: [],
    attempts: [],
  });
  await page.route("**/api/agent-runs/session/*", (route) => {
    const session = route.request().url().split("/").at(-1)!;
    return route.fulfill({
      json: {
        success: true,
        data: Array.from({ length: session === "long" ? 12 : 1 }, (_, i) => ({
          agent_run_id: `${session}-${i}`,
          session_id: session,
          created_at: new Date(2026, 0, i + 1).toISOString(),
          state: state(`${session}-${i}`),
        })),
      },
    });
  });
  await page.routeWebSocket("**/api/agent-runs/*/events/ws", (ws) => {
    const id = new URL(ws.url()).pathname.split("/")[3];
    opened += 1;
    streams.set(id, ws);
    ws.onClose(() => closed.add(id));
  });
  const ready = (id: string) =>
    streams.get(id)!.send(
      JSON.stringify({
        type: "ready",
        data: { state: state(id), cursor: null },
      }),
    );
  await page.goto("/?history");
  await expect
    .poll(() => [...streams.keys()].sort())
    .toEqual(["long-10", "long-11"]);
  await expect(page.getByTestId("loading")).toHaveText("true");
  ready("long-10");
  ready("long-11");
  await expect(page.getByTestId("loading")).toHaveText("false");
  await expect(page.getByTestId("loaded")).toHaveText("2");
  await expect(page.getByTestId("history")).toHaveText("true");
  await expect.poll(() => streams.size).toBe(5);
  // Slow old logs do not hold the initial conversation or launch the whole history.
  await page.waitForTimeout(150);
  expect(streams.size).toBe(5);
  for (const id of ["long-9", "long-8", "long-7"]) ready(id);
  await expect.poll(() => streams.size).toBe(8);
  for (const id of ["long-6", "long-5", "long-4"]) ready(id);
  await expect.poll(() => streams.size).toBe(11);
  for (const id of ["long-3", "long-2", "long-1"]) ready(id);
  await expect.poll(() => streams.size).toBe(12);
  ready("long-0");
  await expect(page.getByTestId("loaded")).toHaveText("12");
  await expect(page.getByTestId("history")).toHaveText("false");
  await expect.poll(() => closed.size).toBe(12);
  await page.waitForTimeout(2500);
  expect(opened).toBe(12); // Discovery must not reconnect settled history.
  await page.getByRole("button", { name: "Other session" }).click();
  await expect.poll(() => streams.has("other-0")).toBe(true);
  await expect(page.getByTestId("loaded")).toHaveText("0");
  ready("other-0");
  await expect(page.getByTestId("loading")).toHaveText("false");
  await expect(page.getByTestId("loaded")).toHaveText("1");
  await expect(page.getByTestId("session")).toHaveText("other");
});

test("an older active run is prioritised alongside recent messages", async ({
  page,
}) => {
  const streams = new Map<string, WebSocketRoute>();
  const state = (id: string) => ({
    agent_run_id: id,
    status: id === "run-0" ? "awaiting_input" : "succeeded",
    projection_status: "current",
    last_run_attempt_number: 1,
    last_event_sequence: 1,
    updated_at: "2026-11-06T00:00:00Z",
  });
  await page.route("**/api/agent-runs/session/*", (route) =>
    route.fulfill({
      json: {
        success: true,
        data: Array.from({ length: 8 }, (_, i) => ({
          agent_run_id: `run-${i}`,
          session_id: "long",
          created_at: new Date(2026, 0, i + 1).toISOString(),
          state: state(`run-${i}`),
        })),
      },
    }),
  );
  await page.routeWebSocket("**/api/agent-runs/*/events/ws", (ws) =>
    streams.set(new URL(ws.url()).pathname.split("/")[3], ws),
  );
  await page.goto("/?history");
  await expect
    .poll(() => [...streams.keys()].sort())
    .toEqual(["run-0", "run-6", "run-7"]);
  for (const id of ["run-6", "run-7"])
    streams.get(id)!.send(
      JSON.stringify({
        type: "ready",
        data: { state: state(id), cursor: null },
      }),
    );
  await expect(page.getByTestId("loading")).toHaveText("true");
  streams.get("run-0")!.send(
    JSON.stringify({
      type: "ready",
      data: { state: state("run-0"), cursor: null },
    }),
  );
  await expect(page.getByTestId("loading")).toHaveText("false");
  await expect(page.getByTestId("history")).toHaveText("true");
});

test("terminal snapshots reopen for retry, recover disconnects, and settle again", async ({
  page,
}) => {
  let attempt = 1;
  let status = "failed";
  const state = () => ({
    agent_run_id: "retry-run",
    status,
    projection_status: "current",
    last_run_attempt_number: attempt,
    last_event_sequence: 1,
    updated_at: `2026-11-06T00:00:0${attempt}Z`,
  });
  await page.route("**/api/agent-runs/session/*", (route) =>
    route.fulfill({
      json: {
        success: true,
        data: [
          {
            agent_run_id: "retry-run",
            session_id: "long",
            created_at: "2026-11-06T00:00:00Z",
            state: state(),
          },
        ],
      },
    }),
  );
  const streams: WebSocketRoute[] = [];
  let closed = 0;
  await page.routeWebSocket("**/api/agent-runs/*/events/ws*", (ws) => {
    streams.push(ws);
    ws.onClose(() => {
      closed += 1;
    });
    ws.send(
      JSON.stringify({ type: "ready", data: { state: state(), cursor: null } }),
    );
  });
  await page.goto("/?history");
  await expect.poll(() => closed).toBe(1);
  await expect(page.getByTestId("loaded")).toHaveText("1");
  attempt = 2;
  status = "running";
  await expect.poll(() => streams.length).toBe(2);
  // A transport close while active must still reconnect.
  streams[1].close();
  await expect.poll(() => streams.length).toBe(3);
  status = "cancelled";
  streams[2].send(
    JSON.stringify({ type: "state", data: { state: state(), cursor: null } }),
  );
  await expect.poll(() => closed).toBe(3);
  // Retry may start AND finish between discovery polls.
  attempt = 3;
  status = "succeeded";
  await expect.poll(() => streams.length).toBe(4);
  await expect.poll(() => closed).toBe(4);
  await page.waitForTimeout(2500);
  expect(streams.length).toBe(4);
  await expect(page.getByTestId("loaded")).toHaveText("1");
});
