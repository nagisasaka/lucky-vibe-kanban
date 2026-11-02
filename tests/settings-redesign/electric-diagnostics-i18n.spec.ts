import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Sync diagnostics JA→EN: draft, scoped insert and explicit original diagnostics", async ({
  page,
}) => {
  await page.goto("/?view=electric-diagnostics&lang=ja&diagnostic-mode=error");
  await expect(
    page.getByRole("heading", { name: "Electric SDKのテスト", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "組織", exact: true })
    .selectOption("raw-organization");
  await page.getByRole("button", { name: "接続", exact: true }).click();
  const raw = page.getByText("Raw diagnostic 原文 / internal-test-path", {
    exact: true,
  });
  await expect(raw).toBeHidden();
  await page.getByText("診断の詳細（原文）", { exact: true }).click();
  await expect(raw).toBeVisible();
  await page.getByRole("button", { name: "再試行", exact: true }).click();
  await expect(raw).toHaveCount(0);
  await expect(
    page.getByText("High: raw projects", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("textbox", { name: "名前", exact: true })
    .fill("Raw 診断 project");
  await page
    .getByRole("textbox", { name: "プロジェクトキー", exact: true })
    .fill("RAW2");
  await switchLanguage(page, "en");
  await expect(
    page.getByRole("textbox", { name: "Name", exact: true }),
  ).toHaveValue("Raw 診断 project");
  const writes = () =>
    page.evaluate(
      () =>
        (window as unknown as { __electricMutations: unknown[] })
          .__electricMutations,
    );
  expect(await writes()).toEqual([]);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect.poll(writes).toEqual([
    {
      table: "projects",
      scope: { organization_id: "raw-organization" },
      operation: "insert",
      value: {
        organization_id: "raw-organization",
        name: "Raw 診断 project",
        key: "RAW2",
        color: "#3b82f6",
      },
    },
  ]);
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Connect", exact: true }),
  ).toBeVisible();
});
