import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("priority selection keeps the canonical value through locale change and cancel", async ({
  page,
}) => {
  await page.goto("/?view=action-labels&lang=ja");
  const dialog = page.getByRole("dialog");
  const result = page.getByTestId("selection-result");
  await page.getByTestId("select-priority").click();
  await switchLanguage(page, "en");
  await dialog.getByRole("option", { name: "High", exact: true }).click();
  await expect(result).toHaveText('{"priority":"high"}');
  await expect(dialog).toHaveCount(0);
  await page.getByTestId("select-priority").click();
  await expect(dialog.getByRole("combobox")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(result).toHaveText("cancelled");
});
