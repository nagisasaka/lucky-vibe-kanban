import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Command search and Projects guide JA→EN retain current state", async ({
  page,
}) => {
  await page.goto("/?view=action-labels&lang=ja");
  const search = page.getByRole("textbox", { name: "fixture-search" });
  const pin = page.locator('[data-search-action="pin-workspace"]');
  await search.fill("ピン留めを解除");
  await expect(pin).toHaveText("ピン留めを解除", { timeout: 15000 });
  await switchLanguage(page, "en");
  await expect(search).toHaveValue("ピン留めを解除");
  await expect(pin).toHaveCount(0);
  await search.fill("Unpin");
  await expect(pin).toHaveText("Unpin");
  await page.getByTestId("projects-guide").click();
  await page
    .getByRole("button", { name: "Shared server access", exact: true })
    .click();
  const heading = (name: string) =>
    page.getByRole("heading", { name, exact: true });
  await expect(heading("Shared server access").locator("..")).toContainText(
    "when your permissions allow it",
  );
  await switchLanguage(page, "ja");
  const content = heading("共有サーバーへのアクセス").locator("..");
  await expect(content).toContainText("メンバー権限で管理されます");
  await expect(content).not.toContainText("Cloud");
  await expect(content.locator("img")).toHaveCount(0);
  await page.getByRole("button", { name: "閉じる", exact: true }).click();
  await expect(heading("共有サーバーへのアクセス")).toHaveCount(0);
});

test(`Action delete confirmation JA→EN: cancel, live locale and exact targets`, async ({
  page,
}) => {
  await page.goto("/?view=action-labels&lang=ja");
  await page.getByTestId("delete-two").click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("heading")).toHaveText("2件のIssueを削除");
  await expect(dialog).toContainText("この操作は取り消せません。");
  await dialog.getByRole("button", { name: "キャンセル", exact: true }).click();
  await expect(page.getByTestId("deleted-ids")).toHaveText("[]");
  await page.getByTestId("delete-two").click();
  await switchLanguage(page, "en");
  await expect(dialog.getByRole("heading")).toHaveText("Delete 2 Issues");
  await page.setViewportSize({ width: 360, height: 800 });
  await expectWithinWidth(dialog, 360);
  await dialog.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId("deleted-ids")).toHaveText(
    '["raw-issue-1","raw-issue-2"]',
  );
});
