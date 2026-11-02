import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`release notes and comment dates use ${language} while external content stays raw`, async ({
    page,
  }) => {
    await page.goto(`/?view=release-notes&lang=${language}`);
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByRole("heading", {
        name: ja ? "更新情報" : "What’s New",
        exact: true,
      }),
    ).toBeVisible();
    await expect(page.getByRole("dialog")).toContainText(
      "Raw release content 原文",
    );
    const date = await page.evaluate(
      (locale) =>
        new Date("2026-01-02T15:04:05Z").toLocaleDateString(locale, {
          year: "numeric",
          month: "short",
          day: "numeric",
        }),
      language,
    );
    await expect(page.getByRole("dialog")).toContainText(date);
    await expect(
      page.getByRole("button", {
        name: ja ? "GitHubを開く" : "Open on GitHub",
        exact: true,
      }),
    ).toBeVisible();
    await page.evaluate(() => {
      let calls = 0;
      window.open = () => {
        calls++;
        return null;
      };
      Object.defineProperty(window, "__fixtureOpenCalls", { get: () => calls });
    });
    await page.keyboard.press("Enter");
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __fixtureOpenCalls: number })
            .__fixtureOpenCalls,
      ),
    ).toBe(1);
    await expect(page.getByRole("dialog")).toBeVisible();
    await page
      .getByRole("button", { name: ja ? "閉じる" : "Close", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.goto(
      `/?view=release-notes&lang=${language}&releasesState=error`,
    );
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByText(
        ja
          ? "リリースノートを読み込めませんでした。"
          : "Unable to load release notes.",
        { exact: true },
      ),
    ).toBeVisible();
    await expect(page.getByRole("dialog")).not.toContainText("fixture private");
    await page.goto(`/?view=formatting&lang=${language}`);
    await expect(
      page.getByText("@Raw Author 原文", { exact: true }),
    ).toBeVisible();
    const timestamp = await page.evaluate(
      (locale) => new Date("2026-01-02T15:04:05Z").toLocaleString(locale),
      language,
    );
    await expect(page.getByText(timestamp, { exact: true })).toBeVisible();
    const other = ja ? "en" : "ja";
    await switchLanguage(page, other);
    const switched = await page.evaluate(
      (locale) => new Date("2026-01-02T15:04:05Z").toLocaleString(locale),
      other,
    );
    await expect(page.getByText(switched, { exact: true })).toBeVisible();
    await expect(
      page.getByText("Raw comment 原文", { exact: true }),
    ).toBeVisible();
  });
}
