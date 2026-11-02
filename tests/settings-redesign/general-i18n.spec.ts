import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`general settings ${language}: mobile scale labels preserve immediate device storage`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 360, height: 900 });
    await page.goto(`/?lang=${language}&section=general`);
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByText(ja ? "モバイルの文字サイズ" : "Mobile Font Size", {
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: ja ? "標準（100%）" : "Default (100%)",
        exact: true,
      })
      .click();
    await page
      .getByRole("menuitem", {
        name: ja ? "小さめ（95%）" : "Small (95%)",
        exact: true,
      })
      .click();
    expect(
      await page.evaluate(() => localStorage.getItem("vk-mobile-font-scale")),
    ).toBe("small");
    await expect(
      page.getByRole("button", { name: ja ? "保存" : "Save", exact: true }),
    ).toHaveCount(0);
    await page.reload();
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByRole("button", {
        name: ja ? "小さめ（95%）" : "Small (95%)",
        exact: true,
      }),
    ).toBeVisible();
    await expectNoHorizontalOverflow(page);
    await page.goto(`/?lang=${language}&section=notifications`);
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await page.locator("#sound-enabled").check();
    await expect(
      page.getByRole("button", {
        name: ja ? "通知音を試聴" : "Preview sound",
        exact: true,
      }),
    ).toBeVisible();
  });
}

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
