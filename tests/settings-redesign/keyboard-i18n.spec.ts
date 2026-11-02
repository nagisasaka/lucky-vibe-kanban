import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
test(`Shortcut help JA→EN: live labels, unchanged keys and mobile close`, async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 900 });
  await page.goto(`/?view=keyboard-shortcuts&lang=ja`);
  await page.getByRole("button", { name: "設定を開く", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("設定に移動", { exact: true })).toBeVisible();
  await expect(
    dialog.getByText("インラインコードに整形", {
      exact: true,
    }),
  ).toBeVisible();
  await expectWithinWidth(dialog, 360, 8);
  const keys = await dialog.locator("kbd").allTextContents();
  await switchLanguage(page, "en");
  await expect(
    dialog.getByText("Go to Settings", { exact: true }),
  ).toBeVisible();
  expect(await dialog.locator("kbd").allTextContents()).toEqual(keys);
  await page.setViewportSize({ width: 360, height: 900 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await expectWithinWidth(dialog, 360, 8);
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await page
    .getByRole("button", { name: "設定を開き直す", exact: true })
    .click();
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await expect(dialog).not.toBeVisible();
});
