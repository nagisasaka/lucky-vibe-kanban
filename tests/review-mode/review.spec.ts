import { test, expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

test("real UI/SQLite preserves REVIEW through reload and executes a bounded normal turn", async ({
  page,
  request,
}, testInfo) => {
  const access = await (await request.get("/api/access/status")).json();
  const headers = {
    "x-lvk-csrf": access.csrf,
    Origin: process.env.LVK_E2E_BASE_URL!,
  };
  const api = async (
    url: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(url, { method, data, headers });
    const body = await response.json();
    expect(response.ok(), JSON.stringify(body)).toBeTruthy();
    expect(body.success, JSON.stringify(body)).toBeTruthy();
    return body.data;
  };
  const info = await api("/api/info");
  await api(
    "/api/config",
    {
      ...info.config,
      disclaimer_acknowledged: true,
      onboarding_acknowledged: true,
      analytics_enabled: false,
      show_release_notes: false,
      language: "EN",
      showcases: { seen_features: ["workspaces-guide"] },
    },
    "PUT",
  );
  const directory = join(process.env.LVK_REVIEW_TEST_ROOT!, randomUUID());
  mkdirSync(directory);
  const created = await api("/api/workspaces/start", {
    mode: "direct_folder",
    name: "REVIEW browser fixture",
    repos: [],
    directory_path: directory,
    linked_issue: null,
    attachment_ids: [],
    executor_config: {
      executor: "CODEX",
      execution_mode: "code",
      permission_policy: "SUPERVISED",
    },
    prompt: "Initial CODE fixture",
  });
  const initial = created.agent_run.agent_run_id;
  const session = created.agent_run.state.session_id;
  await expect
    .poll(async () => (await api(`/api/agent-runs/${initial}`)).status)
    .toBe("succeeded");
  await page.goto(`/workspaces/${created.workspace.id}`);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Code", exact: true }).click();
  await page.getByRole("menuitem", { name: "Review", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Review", exact: true }),
  ).toBeVisible();
  await expect
    .poll(async () => JSON.stringify(await api("/api/scratch")))
    .toContain('"execution_mode":"review"');
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Review", exact: true }),
  ).toBeVisible();
  const editor = page
    .locator('[contenteditable="true"][role="textbox"]')
    .first();
  await editor.fill(
    "Review this fixture; investigate only, report findings and stop.",
  );
  const sent = page.waitForResponse(
    (response) =>
      response.url().includes("/follow-up") &&
      response.request().method() === "POST",
  );
  await editor.press("Enter");
  const response = await sent;
  expect(response.ok(), await response.text()).toBeTruthy();
  expect(response.request().postDataJSON().executor_config.execution_mode).toBe(
    "review",
  );
  const run = (await response.json()).data.agent_run_id;
  await expect
    .poll(async () => (await api(`/api/agent-runs/${run}`)).status)
    .toBe("succeeded");
  const saved = await api(`/api/sessions/${session}/executor-config`);
  expect(saved.execution_mode).toBe("review");
  expect(saved.permission_policy).toBe("SUPERVISED");
  const state = await api(`/api/agent-runs/${run}`);
  expect(state.terminal_output.content).toContain("REVIEW fixture findings");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Review", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("REVIEW fixture findings: report and stop.", {
      exact: true,
    }),
  ).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("review-result.png"),
    fullPage: true,
  });
  const traces = readdirSync(process.env.LVK_REVIEW_FIXTURE_TRACE!).flatMap(
    (name) =>
      readFileSync(join(process.env.LVK_REVIEW_FIXTURE_TRACE!, name), "utf8")
        .trim()
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line)),
  );
  const starts = traces.filter((value) =>
    ["thread/start", "thread/resume"].includes(value.method),
  );
  expect(starts.some((value) => value.review)).toBeTruthy();
  const codeStart = starts.find((value) => !value.review);
  expect(codeStart?.approval).toBeTruthy();
  expect(
    starts
      .filter((value) => value.review)
      .every((value) => value.approval === codeStart.approval),
  ).toBeTruthy();
  expect(
    starts
      .filter((value) => value.review)
      .every((value) => value.sandbox === "workspace-write"),
  ).toBeTruthy();
  expect(traces.filter((value) => value.method === "turn/start")).toHaveLength(
    2,
  );
  expect(
    traces.filter((value) => value.method.startsWith("thread/goal/")),
  ).toHaveLength(0);
  expect(
    traces
      .filter((value) => value.method === "turn/start")
      .every((value) => value.collaboration_mode === "default"),
  ).toBeTruthy();
});
