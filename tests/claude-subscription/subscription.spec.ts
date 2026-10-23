import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

const config = {
  executor: "CLAUDE_CODE",
  variant: "SUBSCRIPTION",
  execution_mode: "code",
};
const terminal = new Set([
  "succeeded",
  "failed",
  "cancelled",
  "crashed",
  "audit_failed",
]);
const identity = () => ({
  command_id: randomUUID(),
  idempotency_key: randomUUID(),
  correlation_id: randomUUID(),
  created_at: new Date().toISOString(),
});
function traces() {
  return readdirSync(process.env.LVK_CLAUDE_FIXTURE_TRACE!).flatMap((name) =>
    readFileSync(join(process.env.LVK_CLAUDE_FIXTURE_TRACE!, name), "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line)),
  );
}

test("subscription preflight, real runtime lifecycle and browser output", async ({
  page,
  request,
}) => {
  const access = await (await request.get("/api/access/status")).json();
  const headers = {
    "x-lvk-csrf": access.csrf,
    Origin: process.env.LVK_E2E_BASE_URL!,
  };
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, { method, data, headers });
    const body = await response.json();
    expect(response.ok(), `${path}: ${JSON.stringify(body)}`).toBeTruthy();
    expect(body.success, JSON.stringify(body)).toBeTruthy();
    return body.data;
  };
  const info = await api("/api/info");
  // Disposable local test-instance onboarding only; no provider terms/login.
  await api(
    "/api/config",
    {
      ...info.config,
      disclaimer_acknowledged: true,
      onboarding_acknowledged: true,
      language: "EN",
      analytics_enabled: false,
      show_release_notes: false,
      commit_reminder_enabled: false,
      showcases: { seen_features: ["workspaces-guide"] },
    },
    "PUT",
  );
  const policy = (variant = "SUBSCRIPTION") =>
    api(`/api/agents/profile-policy?executor=CLAUDE_CODE&variant=${variant}`);
  for (const state of [
    { loggedIn: false },
    { authMethod: "api_key" },
    { apiProvider: "bedrock" },
    { malformed: true },
    { exitCode: 1 },
  ]) {
    writeFileSync(process.env.LVK_CLAUDE_FIXTURE_STATE!, JSON.stringify(state));
    const before = traces().filter(
      (v) => v.event === "launch" && !v.discovery,
    ).length;
    const denied = await policy();
    expect(denied.readiness).toBe("AUTH_REQUIRED");
    expect(denied.capabilities).not.toContain("INITIAL_RUN");
    const rejectedDirectory = join(
      process.env.LVK_CLAUDE_TEST_ROOT!,
      randomUUID(),
    );
    mkdirSync(rejectedDirectory);
    const rejected = await request.post("/api/workspaces/start", {
      headers,
      data: {
        mode: "direct_folder",
        name: "Rejected subscription fixture",
        repos: [],
        directory_path: rejectedDirectory,
        linked_issue: null,
        attachment_ids: [],
        executor_config: config,
        prompt: "Must never reach runtime",
      },
    });
    const rejectedBody = await rejected.json();
    if (rejected.ok() && rejectedBody.success) {
      const run = rejectedBody.data.agent_run.agent_run_id;
      await expect
        .poll(async () =>
          terminal.has((await api(`/api/agent-runs/${run}`)).status),
        )
        .toBe(true);
      expect((await api(`/api/agent-runs/${run}`)).status).not.toBe(
        "succeeded",
      );
    } else {
      expect(JSON.stringify(rejectedBody)).toMatch(/auth|login|subscription/i);
    }

    expect(
      traces().filter((v) => v.event === "launch" && !v.discovery),
    ).toHaveLength(before);
  }
  writeFileSync(process.env.LVK_CLAUDE_FIXTURE_STATE!, "{}");
  expect((await policy("CONFLICT")).readiness).toBe("AUTH_REQUIRED");
  const executable = join(
    dirname(process.env.LVK_CLAUDE_FIXTURE_STATE!),
    "bin/claude",
  );
  renameSync(executable, executable + ".hidden");
  try {
    expect((await policy()).readiness).toBe("MISSING_EXECUTABLE");
  } finally {
    renameSync(executable + ".hidden", executable);
  }
  expect((await policy()).readiness).toBe("READY");
  const directory = join(process.env.LVK_CLAUDE_TEST_ROOT!, randomUUID());
  mkdirSync(directory);
  const created = await api("/api/workspaces/start", {
    mode: "direct_folder",
    name: "Claude subscription fixture",
    repos: [],
    directory_path: directory,
    linked_issue: null,
    attachment_ids: [],
    executor_config: config,
    prompt: "Fixture start",
  });
  const workspace = created.workspace.id;
  const first = created.agent_run.agent_run_id;
  const state = (run: string) => api(`/api/agent-runs/${run}`);
  const settled = async (run: string, status: string) => {
    await expect
      .poll(async () => terminal.has((await state(run)).status), {
        timeout: 60000,
      })
      .toBe(true);
    const actual = (await state(run)).status;
    if (actual !== status) {
      const events = (await api(`/api/agent-runs/${run}/events`)).events;
      console.log(
        JSON.stringify({
          run,
          expected: status,
          actual,
          errors: events
            .filter((e: any) => e.payload.type === "error")
            .map((e: any) => e.payload.data),
        }),
      );
    }
    expect(actual).toBe(status);
  };
  await settled(first, "succeeded");
  const session = (await state(first)).session_id;
  await page.goto(`/workspaces/${workspace}`);
  await page.keyboard.press("Escape");
  await expect(
    page.getByText("Fixture started", { exact: true }).first(),
  ).toBeVisible();
  const follow = async (prompt: string) => {
    const result = await api(`/api/sessions/${session}/follow-up`, {
      prompt,
      executor_config: config,
    });
    return result.agent_run_id;
  };
  writeFileSync(
    process.env.LVK_CLAUDE_FIXTURE_STATE!,
    JSON.stringify({ loggedIn: false }),
  );
  const beforeRejectedFollow = traces().filter(
    (v) => v.event === "user",
  ).length;
  const deniedFollow = await request.post(
    `/api/sessions/${session}/follow-up`,
    {
      headers,
      data: { prompt: "Expired login must not run", executor_config: config },
    },
  );
  const deniedFollowBody = await deniedFollow.json();
  if (deniedFollow.ok() && deniedFollowBody.success) {
    const run = deniedFollowBody.data.agent_run_id;
    await expect
      .poll(async () => terminal.has((await state(run)).status))
      .toBe(true);
    expect((await state(run)).status).not.toBe("succeeded");
  } else {
    expect(JSON.stringify(deniedFollowBody)).toMatch(
      /auth|login|subscription/i,
    );
  }
  expect(traces().filter((v) => v.event === "user")).toHaveLength(
    beforeRejectedFollow,
  );
  writeFileSync(process.env.LVK_CLAUDE_FIXTURE_STATE!, "{}");
  const second = await follow("Fixture follow up");
  await settled(second, "succeeded");
  await expect(
    page.getByText("Fixture resumed", { exact: true }).first(),
  ).toBeVisible();
  const failure = await follow("CLAUDE_FIXTURE_FAIL");
  await settled(failure, "failed");
  const held = await follow("CLAUDE_FIXTURE_HOLD");
  await expect.poll(async () => (await state(held)).status).toBe("running");
  await page
    .getByRole("button", { name: /^Stop$/ })
    .last()
    .click();
  await settled(held, "cancelled");
  const resumed = await follow("Fixture resume after stop");
  await settled(resumed, "succeeded");
  const launches = traces().filter((v) => v.event === "launch" && !v.discovery);
  expect(launches.every((v) => v.subscription)).toBe(true);
  const resumedLaunches = launches.filter((v) => v.resume);
  expect(resumedLaunches.length).toBeGreaterThanOrEqual(4);
  expect(new Set(resumedLaunches.map((v) => v.resume)).size).toBe(1);
  await page.screenshot({
    path: test.info().outputPath("claude-subscription.png"),
    fullPage: true,
  });
  console.log(
    JSON.stringify({
      workspace,
      session,
      runs: [first, second, failure, held, resumed],
      nativeResumeIds: [...new Set(resumedLaunches.map((v) => v.resume))],
    }),
  );
});

test("Issue to Workspace selects subscription and sends through the real UI", async ({
  page,
  request,
}) => {
  writeFileSync(process.env.LVK_CLAUDE_FIXTURE_STATE!, "{}");
  const access = await (await request.get("/api/access/status")).json();
  const headers = {
    "x-lvk-csrf": access.csrf,
    Origin: process.env.LVK_E2E_BASE_URL!,
  };
  const api = async (path: string, data?: unknown) => {
    const response = await request.fetch(path, {
      method: data === undefined ? "GET" : "POST",
      data,
      headers,
    });
    const body = await response.json();
    expect(response.ok(), JSON.stringify(body)).toBeTruthy();
    return body.data?.data ?? body.data ?? body;
  };
  const orgs = await api("/api/board/organizations");
  const org = orgs.organizations[0];
  const project = await api("/api/local/v1/projects", {
    organization_id: org.id,
    name: "Claude subscription UI",
    color: "210 80% 52%",
  });
  const statuses = await api(
    `/api/board/project-statuses?project_id=${project.id}`,
  );
  const issue = await api("/api/board/issues", {
    project_id: project.id,
    status_id: statuses.project_statuses[0].id,
    title: "Subscription UI fixture",
    description: "Fixture initial request",
    sort_order: 0,
    extension_metadata: {},
  });
  await page.goto(`/projects/${project.id}/issues/${issue.id}`);
  await page
    .getByRole("button", { name: "New single-agent attempt", exact: true })
    .last()
    .click();
  const chooser = page
    .getByRole("heading", { name: "Choose workspace", exact: true })
    .locator("../..");
  const directory = join(process.env.LVK_CLAUDE_TEST_ROOT!, randomUUID());
  mkdirSync(directory);
  await chooser
    .getByLabel("Working directory", { exact: true })
    .fill(directory);
  await chooser.getByRole("button", { name: "Load path", exact: true }).click();
  await chooser
    .getByRole("button", { name: /Work directly in this directory/ })
    .click();
  await chooser
    .getByRole("button", { name: "Use this directory", exact: true })
    .click();
  // Explicitly choose the provider, even when the user's isolated default differs.
  const provider = page
    .getByRole("button", { name: /^(Codex|Claude Code)$/ })
    .last();
  await provider.click();
  await page
    .getByRole("menuitem", { name: "Claude Code", exact: true })
    .click();
  await page.getByRole("button", { name: "Preset", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "Subscription", exact: true })
    .click();
  const editor = page.locator('[contenteditable="true"]').last();
  await editor.fill("Fixture UI start");
  const started = page.waitForResponse(
    (r) =>
      r.url().endsWith("/api/workspaces/start") &&
      r.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Create", exact: true }).click();
  const response = await started;
  expect(response.ok()).toBeTruthy();
  const submitted = response.request().postDataJSON();
  expect(submitted.executor_config.executor).toBe("CLAUDE_CODE");
  expect(submitted.executor_config.variant).toBe("SUBSCRIPTION");
  expect(submitted.linked_issue.issue_id).toBe(issue.id);
  const created = (await response.json()).data;
  const workspace = created.workspace.id;
  await expect(
    page.getByText("Fixture started", { exact: true }).first(),
  ).toBeVisible();
  await editor.fill("Fixture UI follow up");
  const continued = page.waitForResponse(
    (r) => r.url().endsWith("/follow-up") && r.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Send", exact: true }).last().click();
  expect((await continued).ok()).toBeTruthy();
  await expect(
    page.getByText("Fixture resumed", { exact: true }).first(),
  ).toBeVisible();
  const discoveryUrls: string[] = [];
  page.on("websocket", (socket) => discoveryUrls.push(socket.url()));
  await page.getByRole("button", { name: "Latest", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "New Session", exact: true })
    .click();
  await provider.click();
  await page
    .getByRole("menuitem", { name: "Claude Code", exact: true })
    .click();
  await page.getByRole("button", { name: "Preset", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "Subscription", exact: true })
    .click();
  await expect
    .poll(() =>
      discoveryUrls.some((value) => {
        const url = new URL(value);
        return (
          url.pathname.endsWith("/agents/discovered-options/ws") &&
          url.searchParams.get("executor") === "CLAUDE_CODE" &&
          url.searchParams.get("variant") === "SUBSCRIPTION" &&
          url.searchParams.get("workspace_id") === workspace &&
          !url.searchParams.has("session_id")
        );
      }),
    )
    .toBe(true);
  await page.screenshot({
    path: test.info().outputPath("issue-workspace-subscription.png"),
    fullPage: true,
  });
  console.log(
    JSON.stringify({
      issue: issue.id,
      workspace,
      uiStart: true,
      uiFollowUp: true,
    }),
  );
});
