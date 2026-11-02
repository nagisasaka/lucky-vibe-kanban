import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
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
  await dialog.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId("deleted-ids")).toHaveText(
    '["raw-issue-1","raw-issue-2"]',
  );
});
