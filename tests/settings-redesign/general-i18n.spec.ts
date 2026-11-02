import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

for (const language of ["en", "ja"]) {
  const ja = language === "ja";
  test(`Browser notification hint ${language}: follows explicit save and preserves Browser preference`, async ({
    page,
  }) => {
    await page.addInitScript((language) => {
      Object.defineProperty(navigator, "languages", {
        configurable: true,
        get: () => ["de-DE", language === "ja" ? "ja-JP" : "en-US"],
      });
      Object.defineProperty(navigator, "language", {
        configurable: true,
        get: () => "de-DE",
      });
    }, language);
    await page.goto(
      `/?lang=${language}&section=notifications&browserLanguage=true&web-push-load-error`,
    );
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await page.locator("#sound-enabled").check();
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __fixtureLocaleSaves: unknown[] })
            .__fixtureLocaleSaves.length,
      ),
    ).toBe(0);
    await page
      .getByRole("button", { name: ja ? "保存" : "Save", exact: true })
      .click();
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (window as unknown as { __fixtureLocaleSaves: unknown[] })
              .__fixtureLocaleSaves.length,
        ),
      )
      .toBe(1);
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __fixtureLocaleSaves: unknown[] })
            .__fixtureLocaleSaves[0],
      ),
    ).toEqual({ language: "BROWSER", browserLanguage: language });
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Web Pushの登録を読み込めませんでした。再実行する前に接続と権限を確認してください。"
        : "Could not load Web Push registrations. Check your connection and access before trying again.",
    );
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Could not load Web Push registrations. Check your connection and access before trying again."
        : "Web Pushの登録を読み込めませんでした。再実行する前に接続と権限を確認してください。",
    );
    await expect(page.locator("body")).not.toContainText(
      "Private push fixture endpoint secret",
    );
    expect(
      await page.evaluate(
        () =>
          (
            window as unknown as { __settingsFixtureCalls: string[] }
          ).__settingsFixtureCalls.filter((path) => path === "/api/web-push")
            .length,
      ),
    ).toBe(1);
  });
}
