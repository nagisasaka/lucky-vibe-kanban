import { expect, test, type WebSocketRoute } from "@playwright/test";

test("recent conversation displays before older replay; background batches and session isolation", async ({
  page,
}) => {
  const streams = new Map<string, WebSocketRoute>();
  const state = (id: string) => ({
    agent_run_id: id,
    status: "succeeded",
    projection_status: "current",
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
    streams.set(new URL(ws.url()).pathname.split("/")[3], ws);
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
