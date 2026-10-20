import { expect, test } from "@playwright/test";

// Disposable instance only. Workspaces, Sessions and Scratch use the real API/DB.
// Provider discovery and turn execution are synthetic: no paid model is called.
test("manual reasoning survives discovery, send, reload and workspace switches", async ({
  page,
  request,
}) => {
  test.skip(
    process.env.LVK_E2E_REASONING !== "1",
    "Requires a task-owned isolated instance",
  );
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, { method, data });
    expect(response.ok(), await response.text()).toBeTruthy();
    const body = await response.json();
    expect(body.success, JSON.stringify(body)).toBe(true);
    return body.data;
  };
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
  const scopes = [];
  for (const name of ["Reasoning A", "Reasoning B"]) {
    const workspace = await api("/api/workspaces", { name });
    const session = await api("/api/sessions", {
      workspace_id: workspace.id,
      executor: "CODEX",
      name,
    });
    scopes.push({ workspace, session });
  }
  const [a, b] = scopes;
  const launch: Record<string, any> = {
    [a.session.id]: {
      executor: "CODEX",
      variant: null,
      model_id: "test-model",
      reasoning_id: "max",
    },
    [b.session.id]: {
      executor: "CODEX",
      variant: null,
      model_id: "test-model",
      reasoning_id: null,
    },
  };
  // A pre-fix partial draft still belongs to the inherited launch model.
  await api(
    `/api/scratch/DRAFT_FOLLOW_UP/${a.session.id}`,
    {
      payload: {
        type: "DRAFT_FOLLOW_UP",
        data: {
          message: "",
          executor_config: {
            executor: "CODEX",
            variant: null,
            reasoning_id: "high",
          },
        },
      },
    },
    "PUT",
  );
  const discoverySockets: Array<any> = [];
  const discovery = (efforts: string[]) => ({
    model_selector: {
      providers: [],
      models: [
        {
          id: "test-model",
          name: "Test Model",
          provider_id: null,
          reasoning_options: efforts.map((id) => ({
            id,
            label: id === "high" ? "High" : "Max",
            is_default: id === "high",
          })),
        },
      ],
      default_model: "test-model",
      agents: [],
      permissions: [],
    },
    slash_commands: [],
    skills: [],
    skill_errors: [],
    loading_models: false,
    loading_agents: false,
    loading_slash_commands: false,
    loading_skills: false,
    error: null,
  });
  let efforts = ["high", "max"];
  const publish = (socket: any) =>
    socket.send(
      JSON.stringify({
        JsonPatch: [
          { op: "replace", path: "/options", value: discovery(efforts) },
        ],
      }),
    );
  await page.routeWebSocket(
    "**/api/agents/discovered-options/ws?*",
    (socket) => {
      discoverySockets.push(socket);
      publish(socket);
      socket.send(JSON.stringify({ Ready: true }));
    },
  );
  await page.route("**/api/sessions/*/executor-config", (route) =>
    route.fulfill({
      json: {
        success: true,
        data: launch[route.request().url().split("/").at(-2)!],
      },
    }),
  );
  const sent: any[] = [];
  let finish: (() => void) | undefined;
  await page.route("**/api/sessions/*/follow-up", async (route) => {
    const payload = route.request().postDataJSON();
    sent.push(payload);
    await new Promise<void>((resolve) => {
      finish = resolve;
    });
    launch[route.request().url().split("/").at(-2)!] = payload.executor_config;
    await route.fulfill({ json: { success: true, data: {} } });
  });
  const open = async (scope: typeof a) => {
    await page.goto(
      `/workspaces/${scope.workspace.id}?sessionId=${scope.session.id}`,
    );
    await page.keyboard.press("Escape");
    await expect(
      page.getByRole("button", { name: /Test Model/ }).first(),
    ).toBeVisible();
  };
  const modelButton = () =>
    page.getByRole("button", { name: /Test Model/ }).first();
  const choose = async (label: string) => {
    await modelButton().click();
    await page
      .locator("[data-model-selector-dropdown]")
      .waitFor({ state: "hidden" });
    await page
      .getByRole("button", { name: /^(High|Max|Follow Settings)$/ })
      .click();
    await page.getByRole("menuitem", { name: label, exact: true }).click();
    await page.keyboard.press("Escape");
  };
  const scratch = () => api(`/api/scratch/DRAFT_FOLLOW_UP/${a.session.id}`);
  await open(a);
  await expect(modelButton()).toContainText("High");
  await choose("Max");
  await expect(modelButton()).toContainText("Max");
  await expect
    .poll(async () => (await scratch())?.payload?.data?.executor_config)
    .toMatchObject({ model_id: "test-model", reasoning_id: "max" });
  // Discovery omission must neither mutate Scratch nor change the explicit label.
  efforts = [];
  discoverySockets.forEach(publish);
  await expect(modelButton()).toContainText("Max");
  await modelButton().click();
  await page.getByRole("button", { name: "Max", exact: true }).click();
  await expect(
    page.getByRole("menuitem", { name: "Max", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  efforts = ["high"];
  discoverySockets.forEach(publish);
  await expect(modelButton()).toContainText("Max");
  await open(b);
  await expect(modelButton()).not.toContainText("Max");
  await open(a);
  await expect(modelButton()).toContainText("Max");
  await page.reload();
  await expect(modelButton()).toContainText("Max");
  const editor = page
    .locator('[contenteditable="true"][role="textbox"]')
    .first();
  await editor.fill("Synthetic response for reasoning regression");
  await page.getByRole("button", { name: /^Send( message)?$/ }).click();
  await expect.poll(() => sent.length).toBe(1);
  expect(sent[0].executor_config).toMatchObject({
    model_id: "test-model",
    reasoning_id: "max",
  });
  await expect(modelButton()).toContainText("Max");
  finish!();
  await expect(editor).toHaveText("");
  await expect(modelButton()).toContainText("Max");
  await page.reload();
  await expect(modelButton()).toContainText("Max");
  await choose("Follow Settings");
  await expect
    .poll(
      async () =>
        (await scratch())?.payload?.data?.executor_config.reasoning_id,
    )
    .toBeNull();
  await page.reload();
  await expect(modelButton()).not.toContainText("Max");
  await editor.fill("Synthetic inherited response");
  await page.getByRole("button", { name: /^Send( message)?$/ }).click();
  await expect.poll(() => sent.length).toBe(2);
  expect(sent[1].executor_config.reasoning_id).toBeNull();
  finish!();
  await expect(editor).toHaveText("");
  await open(b);
  await open(a);
  await expect(modelButton()).not.toContainText("Max");
  await modelButton().click();
  await expect(
    page.getByRole("button", { name: "Follow Settings", exact: true }),
  ).toBeVisible();
});
