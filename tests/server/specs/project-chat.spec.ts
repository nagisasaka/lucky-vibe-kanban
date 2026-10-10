import { expect, test } from "@playwright/test";

const project = "10000000-0000-0000-0000-000000000001";
const workspace = "20000000-0000-0000-0000-000000000002";
const issue = "30000000-0000-0000-0000-000000000003";
const session = "40000000-0000-0000-0000-000000000004";
const run = "50000000-0000-0000-0000-000000000005";

test("Astra restores history, accepts another request while running, answers exact questions and opens existing work", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
    console.error(error.message);
  });
  test.skip(
    !process.env.LVK_E2E_SOURCE_ROOT,
    "Requires a disposable development Vite server",
  );
  const requests: Record<string, unknown>[] = [];
  const answers: Record<string, unknown>[] = [];
  let pending = true;
  const messages = [
    {
      sequence: 1,
      id: "message-1",
      conversation_id: "conversation",
      role: "user",
      content: "Implement the agreed feature",
      intent: "implement",
      status: "done",
      error: null,
      created_at: "2026-10-10 00:00:00",
    },
    {
      sequence: 2,
      id: "message-2",
      conversation_id: "conversation",
      role: "assistant",
      content: "Implementation was accepted; tests and merge are unverified.",
      intent: "discuss",
      status: "done",
      error: null,
      created_at: "2026-10-10 00:00:01",
    },
  ];
  await page.route("**/api/projects/*/chat*", async (route) => {
    if (route.request().method() === "POST") {
      const body = route.request().postDataJSON();
      requests.push(body);
      messages.push({
        sequence: messages.length + 1,
        id: body.request_id,
        conversation_id: "conversation",
        role: "user",
        content: body.content,
        intent: body.intent,
        status: "pending",
        error: null,
        created_at: "2026-10-10 00:00:02",
      });
      await route.fulfill({ json: { success: true, data: body.request_id } });
      return;
    }
    const after = Number(
      new URL(route.request().url()).searchParams.get("after") ?? 0,
    );
    await route.fulfill({
      json: {
        success: true,
        data: {
          conversation_id: "conversation",
          configured: true,
          messages: messages.filter((m) => m.sequence > after),
          project_state: {
            issues: [
              { id: issue, title: "Agreed feature", simple_id: "LOCAL-1" },
            ],
            workspaces: [
              { id: workspace, issue_id: issue, name: "Feature attempt" },
            ],
            runs: [
              { agent_run_id: run, workspace_id: workspace, status: "running" },
            ],
            questions: pending
              ? [
                  {
                    id: "attempt:question-message:0",
                    session_id: session,
                    agent_run_id: run,
                    workspace_id: workspace,
                    title: "Which behaviour?",
                    options: ["Preserve", "Replace"],
                    delivery_status: null,
                  },
                ]
              : [],
          },
        },
      },
    });
  });
  await page.route("**/api/sessions/*/questions/answer", async (route) => {
    expect(new URL(route.request().url()).pathname).toBe(
      `/api/sessions/${session}/questions/answer`,
    );
    answers.push(route.request().postDataJSON());
    pending = false;
    await route.fulfill({
      json: { success: true, data: { accepted: true, agent_run_id: run } },
    });
  });
  await page.route("**/__astra-chat-fixture", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: `<!doctype html><html><body><div id="fixture"></div><script type="module">
    import RefreshRuntime from '/@react-refresh';
    RefreshRuntime.injectIntoGlobalHook(window); window.$RefreshReg$=()=>{}; window.$RefreshSig$=()=>type=>type; window.__vite_plugin_react_preamble_installed__=true;
    const fixture=await import('/@fs${process.env.LVK_E2E_SOURCE_ROOT}/tests/server/fixtures/project-chat.tsx'); fixture.mount(); document.body.dataset.fixtureReady='true';
  </script></body></html>`,
    }),
  );
  await page.goto("/__astra-chat-fixture");
  // Cold Vite dependency transforms are setup, separate from UI response time.
  await page.waitForFunction(
    () => document.body.dataset.fixtureReady === "true",
    undefined,
    { timeout: 90_000 },
  );
  await expect(
    page.getByText(
      "Implementation was accepted; tests and merge are unverified.",
    ),
  ).toBeVisible();
  // The project conversation must preserve Japanese input while its state polls,
  // just as the Workspace composer does while execution output arrives.
  const composer = page.getByLabel("Message to Astra");
  await composer.fill("相談中の入力");
  await page.waitForResponse(
    (response) =>
      response.url().includes(`/api/projects/${project}/chat`) &&
      response.request().method() === "GET",
  );
  await composer.dispatchEvent("keydown", {
    key: "Enter",
    code: "Enter",
    isComposing: true,
  });
  await expect(composer).toHaveValue("相談中の入力");
  await expect(composer).toBeFocused();
  expect(requests).toHaveLength(0);
  await page
    .getByLabel("Message to Astra")
    .fill("Discuss another idea while the agent runs");
  await page.getByRole("button", { name: "送信 / Send", exact: true }).click();
  await expect.poll(() => requests.length).toBe(1);
  expect(requests[0].intent).toBe("discuss");
  await page.getByLabel("Message to Astra").fill("And another idea");
  await page.getByRole("button", { name: "送信 / Send", exact: true }).click();
  await expect.poll(() => requests.length).toBe(2);
  expect(requests[0].request_id).not.toBe(requests[1].request_id);
  await page.getByRole("button", { name: /unanswered|未回答/ }).click();
  await page.getByRole("button", { name: "Preserve", exact: true }).click();
  expect(answers).toHaveLength(0);
  await page.getByRole("button", { name: /Send answer|回答を送信/ }).click();
  await expect.poll(() => answers.length).toBe(1);
  expect(answers[0]).toMatchObject({
    question_id: "attempt:question-message:0",
    answer: "Preserve",
  });
  await page.reload();
  await expect(
    page.getByText("Discuss another idea while the agent runs"),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /unanswered|未回答/ }),
  ).toHaveCount(0);
  await page
    .getByText("Issue・Workspace・承認・差分 / Open existing work")
    .click();
  await page.getByRole("button", { name: /Feature attempt/ }).click();
  await expect(page.getByLabel("Navigation")).toHaveText(
    `goToProjectIssueWorkspace:${project}:${issue}:${workspace}`,
  );
  expect(pageErrors).toEqual([]);
});
