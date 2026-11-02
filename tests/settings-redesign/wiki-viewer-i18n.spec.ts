import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Wiki JA→EN: original metadata, selection and scoped read-only reload", async ({
  page,
}) => {
  await page.goto("/?view=wiki-viewer&lang=ja");
  await expect(
    page.getByRole("heading", { name: "Raw home 原文", exact: true }),
  ).toBeVisible({ timeout: 15000 });
  await page
    .getByRole("navigation", { name: "Wikiページ", exact: true })
    .getByRole("button", { name: "Raw title 原文", exact: true })
    .click();
  const article = page.getByRole("region");
  await expect(
    article.getByRole("heading", { name: "Raw article 原文", exact: true }),
  ).toBeVisible();
  await expect(article).toContainText("言語: ja");
  await expect(article).toContainText("更新: 2026-01-02");
  await expect(
    article.getByRole("link", { name: "LVK-7", exact: true }),
  ).toHaveAttribute("href", "/projects/raw-project/issues/raw-issue");
  await page
    .getByRole("searchbox", { name: "Wikiを検索", exact: true })
    .fill("Raw title");
  await switchLanguage(page, "en");
  await expect(
    page.getByRole("searchbox", { name: "Search Wiki", exact: true }),
  ).toHaveValue("Raw title");
  await expect(article).toHaveAccessibleName("Wiki article");
  for (const original of [
    "Raw model documentation {{value}}",
    "Raw summary 原文",
    "feature/raw-branch",
    "openwiki/",
    "Language: ja",
    "Read-only",
  ])
    await expect(article).toContainText(original);
  await article
    .getByRole("button", { name: "Reload Wiki files", exact: true })
    .click();
  await expect(
    article.getByRole("heading", { name: "Raw article 原文", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "Wiki repository", exact: true })
    .selectOption("raw-repo-two");
  await expect(
    article.getByRole("heading", { name: "Raw home 原文", exact: true }),
  ).toBeVisible();
  const calls = await page.evaluate(
    () =>
      (window as unknown as { __settingsFixtureCalls: string[] })
        .__settingsFixtureCalls,
  );
  expect(
    calls
      .filter((path) => path.includes("wiki"))
      .every((path) => path === "/api/workspaces/raw-wiki-workspace/wiki"),
  ).toBe(true);
  expect(calls.some((path) => /initialize|sync|maintenance/.test(path))).toBe(
    false,
  );
});

test("Wiki load failure JA→EN withholds internal details", async ({ page }) => {
  await page.goto("/?view=wiki-viewer&lang=ja&wiki-mode=error");
  await expect(
    page.getByRole("region", { name: "Wikiの記事", exact: true }),
  ).toContainText("Wikiファイルを読み込めませんでした。");
  await switchLanguage(page, "en");
  await expect(
    page.getByRole("region", { name: "Wiki article", exact: true }),
  ).toContainText("Wiki files could not be loaded.");
  await expect(page.locator("body")).not.toContainText("Raw internal secret");
});
