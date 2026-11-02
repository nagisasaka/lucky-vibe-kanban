import { expect, test } from "@playwright/test";

test("release-note failures withhold raw response details", async ({
  page,
}) => {
  await page.goto("/?view=release-notes&lang=en");
  await page.getByRole("button", { name: "設定を開く", exact: true }).click();
  await expect(
    page.getByText("Unable to load release notes.", { exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("dialog")).not.toContainText("fixture private");
});

test("sync-error disclosure withholds private diagnostics", async ({
  page,
}) => {
  await page.goto("/?view=navbar&lang=en");
  await page.getByRole("button", { name: "1 sync error", exact: true }).click();
  await expect(page.getByText("raw-table", { exact: true })).toBeVisible();
  await expect(page.locator("body")).not.toContainText("Raw internal secret");
});
