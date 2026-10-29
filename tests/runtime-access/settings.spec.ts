import { expect, test } from "@playwright/test";
// No live settings, credentials or database: requests exercise the real API client
// against an in-memory route fixture. Rust HTTP tests cover server authorisation/storage.
test("independent permissions, multiline roots, persistence, errors and discard", async ({
  page,
}) => {
  let saved = {
    revision: 0,
    source: "default",
    settings: {
      files_enabled: true,
      execution_enabled: true,
      file_roots: ["/"],
    },
  };
  const writes: (typeof saved)[] = [];
  let rejectSave = false;
  await page.route("**/api/runtime-access-settings", async (route) => {
    if (route.request().method() === "PUT") {
      if (rejectSave)
        return route.fulfill({
          status: 409,
          body: "Settings changed; reload before saving",
        });
      const input = route.request().postDataJSON();
      saved = { ...input, revision: input.revision + 1, source: "saved" };
      writes.push(structuredClone(saved));
    }
    await route.fulfill({ json: saved });
  });
  const files = page.getByRole("checkbox", {
    name: "MCP経由のファイル読み書きを許可",
    exact: true,
  });
  const execution = page.getByRole("checkbox", {
    name: "MCP経由のプログラム実行を許可",
    exact: true,
  });
  const save = page.getByRole("button", { name: "保存", exact: true });
  await page.goto("/");
  await expect(files).toBeChecked();
  await expect(execution).toBeChecked();
  await page.getByText("読み書き範囲を制限する", { exact: true }).click();
  const roots = page.getByRole("textbox", {
    name: "許可root（絶対パス、1行に1件）",
  });
  await expect(roots).toHaveValue("/");
  await expect(
    page.getByText("これはサンドボックスではありません。", { exact: false }),
  ).toBeVisible();
  for (const [f, e] of [
    [true, false],
    [false, true],
    [false, false],
    [true, true],
  ]) {
    await files.setChecked(f);
    await execution.setChecked(e);
    await save.click();
    await expect(save).not.toBeVisible();
    expect(saved.settings.files_enabled).toBe(f);
    expect(saved.settings.execution_enabled).toBe(e);
  }
  await roots.fill("/srv/logs\n/srv/data\n");
  await save.click();
  await expect(save).not.toBeVisible();
  expect(saved.settings.file_roots).toEqual(["/srv/logs", "/srv/data"]);
  await page.reload();
  await expect(roots).toHaveValue("/srv/logs\n/srv/data");
  await files.uncheck();
  await page.getByRole("button", { name: "破棄", exact: true }).click();
  await expect(files).toBeChecked();
  rejectSave = true;
  await execution.uncheck();
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Settings changed");
  await expect(save).toBeVisible();
  expect(writes).toHaveLength(5);
  await page.screenshot({
    path: "test-results/local261/settings.png",
    fullPage: true,
  });
});

test("permission failure does not display editable defaults", async ({
  page,
}) => {
  await page.route("**/api/runtime-access-settings", (route) =>
    route.fulfill({ status: 403, body: "Browser administrator required" }),
  );
  await page.goto("/");
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("checkbox")).toHaveCount(0);
});

test("mobile dark layout keeps both permissions and roots usable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route("**/api/runtime-access-settings", (route) =>
    route.fulfill({
      json: {
        revision: 1,
        source: "saved",
        settings: {
          files_enabled: true,
          execution_enabled: false,
          file_roots: ["/srv/logs", "/srv/data"],
        },
      },
    }),
  );
  await page.goto("/");
  await page.evaluate(() => document.documentElement.classList.add("dark"));
  await expect(page.getByRole("textbox")).toHaveValue("/srv/logs\n/srv/data");
  await expect(page.getByRole("checkbox").first()).toBeChecked();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/local261/settings-mobile-dark.png",
    fullPage: true,
  });
});
