import { expect, test } from "@playwright/test";

// 実アプリ・実APIを使用。fixtureは専用一時DBの設定と空のWorkspaceのみ。
// Agentの起動は行わない。
test("通常遷移・＋・キャンセル・再訪問・サーバーパス選択", async ({
  page,
  request,
}, testInfo) => {
  const directory = process.env.LVK_E2E_DIRECTORY;
  if (!directory)
    throw new Error("テスト専用ディレクトリLVK_E2E_DIRECTORYが必要です");
  const info = await request.get("/api/info");
  expect(info.ok()).toBe(true);
  const config = (await info.json()).data.config;
  const access = await request.get("/api/access/status");
  const csrf = (await access.json()).csrf;
  const headers = { "x-lvk-csrf": csrf };
  const configured = await request.put("/api/config", {
    headers,
    data: {
      ...config,
      onboarding_acknowledged: true,
      disclaimer_acknowledged: true,
      language: "EN",
      analytics_enabled: false,
      showcases: { seen_features: ["workspaces-guide"] },
    },
  });
  expect(configured.ok(), await configured.text()).toBe(true);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  // KeyboardDialog is a portal with a heading, not a native role=dialog.
  const chooser = page
    .getByRole("heading", { name: "Choose workspace", exact: true })
    .locator("../..");
  const add = page.getByRole("button", { name: "New workspace", exact: true });

  await page.goto("/workspaces");
  await expect(
    page.getByText("Select a workspace to get started"),
  ).toBeVisible();
  await expect(chooser).toHaveCount(0);
  await expect(page).toHaveURL(/\/workspaces$/);
  await add.click();
  await expect(chooser).toBeVisible();
  await chooser.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(chooser).toHaveCount(0);
  await add.click();
  await expect(chooser).toBeVisible();

  await chooser
    .getByLabel("Working directory", { exact: true })
    .fill(`${directory}/missing`);
  await chooser.getByRole("button", { name: "Load path", exact: true }).click();
  await expect(chooser.getByRole("alert")).toBeVisible();
  await expect(
    chooser.getByRole("button", { name: "Use this workspace", exact: true }),
  ).toBeDisabled();
  await chooser
    .getByLabel("Working directory", { exact: true })
    .fill(directory);
  await chooser
    .getByRole("button", { name: "Browse server", exact: true })
    .click();
  const picker = page
    .getByRole("heading", { name: "Choose server directory", exact: true })
    .locator("../..");
  await expect(picker).toBeVisible();
  await expect(
    picker.getByRole("button", { name: "child", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("server-picker.png"),
    fullPage: true,
  });
  await expect(picker.getByLabel("Server directory path")).toHaveValue(
    directory,
  );
  await picker.getByLabel("Server directory path").fill(`${directory}/missing`);
  await picker.getByRole("button", { name: "Load path", exact: true }).click();
  await expect(picker.getByRole("alert")).toBeVisible();
  await expect(
    picker.getByRole("button", { name: "Select this directory", exact: true }),
  ).toBeDisabled();
  await picker.getByLabel("Server directory path").fill(directory);
  await picker.getByRole("button", { name: "Load path", exact: true }).click();
  await expect(
    picker.getByRole("button", { name: "child", exact: true }),
  ).toBeVisible();
  await picker.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(chooser).toBeVisible();
  await chooser
    .getByRole("button", { name: "Browse server", exact: true })
    .click();
  await picker.getByRole("button", { name: "child", exact: true }).click();
  await expect(picker.getByLabel("Server directory path")).toHaveValue(
    `${directory}/child`,
  );
  await picker
    .getByRole("button", { name: "Select this directory", exact: true })
    .click();
  await expect(picker).toHaveCount(0);
  await expect(
    chooser.getByLabel("Working directory", { exact: true }),
  ).toHaveValue(`${directory}/child`);
  await chooser
    .getByRole("button", { name: /Work directly in this directory/ })
    .click();
  await chooser
    .getByRole("button", { name: "Use this directory", exact: true })
    .click();
  await expect(chooser).toHaveCount(0);
  await add.click();
  await expect(chooser).toBeVisible();
  await expect(
    chooser.getByLabel("Working directory", { exact: true }),
  ).toHaveValue(`${directory}/child`);
  await chooser.getByRole("button", { name: "Cancel", exact: true }).click();

  // 初期Workspaceを実APIで作る。実行・他Workspaceの変更は不要。
  const created = await request.post("/api/workspaces", {
    headers,
    data: { name: "Navigation fixture" },
  });
  expect(created.ok()).toBe(true);
  const workspaceId = (await created.json()).data.id;
  await page
    .getByRole("button", { name: "Local workspaces", exact: true })
    .click();
  await expect(page).toHaveURL(/\/workspaces$/);
  await expect(
    page.getByText("Navigation fixture", { exact: true }),
  ).toBeVisible();
  await expect(chooser).toHaveCount(0);
  await page.getByText("Navigation fixture", { exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/workspaces/${workspaceId}$`));
  await expect(chooser).toHaveCount(0);
  await page
    .getByRole("button", { name: "Local workspaces", exact: true })
    .click();
  await expect(page).toHaveURL(/\/workspaces$/);
  await add.click();
  await expect(chooser).toBeVisible();
  await chooser.getByRole("button", { name: "Cancel", exact: true }).click();
  await page
    .getByRole("button", { name: "Local workspaces", exact: true })
    .click();
  await page.reload();
  await expect(
    page.getByText("Navigation fixture", { exact: true }),
  ).toBeVisible();
  await expect(chooser).toHaveCount(0);
  await page.screenshot({
    path: testInfo.outputPath("workspace-list.png"),
    fullPage: true,
  });
  expect(errors).toEqual([]);
});

test("日本語・ダークテーマ・狭い画面でサーバー参照できる", async ({
  page,
  request,
}, testInfo) => {
  const directory = process.env.LVK_E2E_DIRECTORY!;
  const config = (await (await request.get("/api/info")).json()).data.config;
  const csrf = (await (await request.get("/api/access/status")).json()).csrf;
  const response = await request.put("/api/config", {
    headers: { "x-lvk-csrf": csrf },
    data: {
      ...config,
      language: "JA",
      theme: "DARK",
      onboarding_acknowledged: true,
      disclaimer_acknowledged: true,
      showcases: { seen_features: ["workspaces-guide"] },
    },
  });
  expect(response.ok(), await response.text()).toBe(true);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/workspaces/create");
  const chooser = page
    .getByRole("heading", { name: "Choose workspace", exact: true })
    .locator("../..");
  await expect(chooser).toBeVisible();
  await chooser
    .getByLabel("Working directory", { exact: true })
    .fill(directory);
  await chooser
    .getByRole("button", { name: "サーバーを参照", exact: true })
    .click();
  const picker = page
    .getByRole("heading", { name: "サーバーのディレクトリを選択", exact: true })
    .locator("../..");
  await expect(
    picker.getByRole("button", { name: "child", exact: true }),
  ).toBeVisible();
  const bounds = await picker.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(390);
  await page.screenshot({
    path: testInfo.outputPath("server-picker-ja-mobile-dark.png"),
    fullPage: true,
  });
  await picker.getByRole("button", { name: "キャンセル", exact: true }).click();
  await expect(chooser).toBeVisible();
});
