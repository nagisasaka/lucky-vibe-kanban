import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Editor JA→EN: command insertion and toolbar preserve original text", async ({
  page,
}) => {
  await page.goto("/?view=editor-presentation&lang=ja");
  const editor = page.getByTestId("editor");
  await editor.fill("/");
  await expect(
    page.getByText("ネイティブコマンド", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Provider 原文 native", { exact: true }),
  ).toBeVisible();
  await switchLanguage(page, "en");
  await expect(
    page.getByText("Native commands", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByTitle("Native commands from the current agent backend", {
      exact: true,
    }),
  ).toBeVisible();
  await page.setViewportSize({ width: 360, height: 800 });
  const menu = page
    .getByText("/raw-skill", { exact: true })
    .locator('xpath=ancestor::div[contains(@style,"position: fixed")][1]');
  await expectWithinWidth(menu, 360);
  await page.getByText("/raw-skill", { exact: true }).click();
  await expect(editor).toHaveText("/raw-skill ");
  const raw = "Raw user text 原文";
  await editor.fill(raw);
  await editor.press("ControlOrMeta+A");
  await expect(
    page.getByRole("button", { name: "Bold (Cmd+B)", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Bold", exact: true }).click();
  await expect(editor.locator("strong")).toHaveText(raw);
  await switchLanguage(page, "ja");
  await expect(
    page.getByRole("button", { name: "インラインコード", exact: true }),
  ).toBeVisible();
  await expect(editor.locator("strong")).toHaveText(raw);
  await expect(editor).toHaveText(raw);
});
