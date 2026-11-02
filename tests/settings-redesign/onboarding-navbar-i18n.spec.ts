import {
  expectNoHorizontalOverflow,
  expectWithinWidth,
  switchLanguage,
} from "./i18n-support";
import { expect, test } from "@playwright/test";
for (const lang of ["en", "ja"]) {
  test(`Onboarding ${lang}: localized controls, draft, saving and original config values`, async ({
    page,
  }) => {
    let ja = lang === "ja";
    await page.goto(`/?view=onboarding&lang=${lang}`);
    await expect(
      page.getByRole("heading", {
        name: ja ? "コーディングエージェント" : "Coding Agent",
        exact: true,
      }),
    ).toBeVisible({ timeout: 15000 });
    await expect
      .poll(() =>
        page
          .locator("img")
          .evaluateAll((images) =>
            images.every(
              (image) =>
                (image as HTMLImageElement).complete &&
                (image as HTMLImageElement).naturalWidth > 0,
            ),
          ),
      )
      .toBe(true);
    await expect(page.locator("code")).toHaveText([
      "--dangerously-skip-permissions",
      "--yolo",
    ]);
    await expect(
      page.getByRole("button", {
        name: ja ? "鶏の鳴き声" : "Rooster",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: ja ? "ドキュメント" : "Docs",
        exact: true,
      }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Codex", exact: true }).click();
    await page
      .getByRole("button", { name: ja ? "音なし" : "No sound", exact: true })
      .click();
    await page
      .getByRole("textbox", {
        name: ja ? "メニュー表示名" : "Menu name",
        exact: true,
      })
      .fill("Raw editor draft 原文");
    await switchLanguage(page, ja ? "en" : "ja");
    ja = !ja;
    await expect(
      page.getByRole("textbox", {
        name: ja ? "メニュー表示名" : "Menu name",
        exact: true,
      }),
    ).toHaveValue("Raw editor draft 原文");
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __onboardingWrites: unknown[] })
            .__onboardingWrites,
      ),
    ).toEqual([]);
    await page.setViewportSize({ width: 360, height: 800 });
    await expectNoHorizontalOverflow(page);
    await page
      .getByRole("button", { name: ja ? "続行" : "Continue", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: ja ? "保存中…" : "Saving...",
        exact: true,
      }),
    ).toBeDisabled();
    const writes = await page.evaluate(
      () =>
        (window as unknown as { __onboardingWrites: Record<string, unknown>[] })
          .__onboardingWrites,
    );
    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({
      onboarding_acknowledged: true,
      disclaimer_acknowledged: true,
      executor_profile: { executor: "CODEX", variant: null },
      editor: {
        entries: [
          {
            name: "Raw editor draft 原文",
            uri_template: "vscode://file/{path}",
          },
        ],
      },
      notifications: { sound_enabled: false, push_enabled: false },
    });
    expect(writes[0]).not.toHaveProperty("language");
    await page.evaluate(() =>
      (
        window as unknown as { __finishOnboarding: () => void }
      ).__finishOnboarding(),
    );
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (window as unknown as { __onboardingNavigations: unknown[] })
              .__onboardingNavigations.length,
        ),
      )
      .toBeGreaterThan(0);
  });

  test(`Onboarding ${lang}: safe errors and live error translation`, async ({
    page,
  }) => {
    const ja = lang === "ja";
    await page.goto(`/?view=onboarding&lang=${lang}&onboarding-error`);
    await page
      .getByRole("button", { name: ja ? "続行" : "Continue", exact: true })
      .click();
    await page.evaluate(() =>
      (
        window as unknown as { __finishOnboarding: () => void }
      ).__finishOnboarding(),
    );
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "保存結果を確認できませんでした。再試行する前に現在の設定を確認してください。"
        : "Saving could not be confirmed. Check the current settings before retrying.",
    );
    await expect(page.locator("body")).not.toContainText("Raw provider secret");
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Saving could not be confirmed. Check the current settings before retrying."
        : "保存結果を確認できませんでした。再試行する前に現在の設定を確認してください。",
    );
    await page.goto(`/?view=onboarding&lang=${lang}&onboarding-loading`);
    await expect(
      page.getByText(ja ? "読み込み中…" : "Loading...", { exact: true }),
    ).toBeVisible();
  });

  test(`Navbar ${lang}: live labels preserve tab IDs and callbacks`, async ({
    page,
  }) => {
    let ja = lang === "ja";
    await page.setViewportSize({ width: 360, height: 800 });
    await page.goto(`/?view=navbar&lang=${lang}`);
    const nav = page.getByRole("navigation");
    await nav
      .getByRole("button", { name: ja ? "ファイル" : "Files", exact: true })
      .click();
    await expect(page.getByTestId("selected-tab")).toHaveText("files");
    await switchLanguage(page, ja ? "en" : "ja");
    ja = !ja;
    await nav
      .getByRole("button", { name: ja ? "差分" : "Diff", exact: true })
      .click();
    await expect(page.getByTestId("selected-tab")).toHaveText("changes");
    for (const label of ja
      ? ["ボード", "設定", "コマンドメニュー", "再読み込み"]
      : ["Board", "Settings", "Command bar", "Reload"])
      await nav.getByRole("button", { name: label, exact: true }).click();
    await expect(page.getByTestId("navbar-events")).toHaveText(
      '["board","settings","command","reload"]',
    );
    await nav
      .getByRole("button", {
        name: ja ? "同期エラー1件" : "1 sync error",
        exact: true,
      })
      .click();
    await expect(page.getByText("raw-table", { exact: true })).toBeVisible();
    await expect(page.locator("body")).not.toContainText("Raw internal secret");
    await expect(
      page.getByText("raw-table", { exact: true }).locator(".."),
    ).toContainText(
      ja
        ? "同期を確認できませんでした。変更を再試行する前に接続と現在のデータを確認してください。"
        : "Synchronization could not be confirmed. Check the connection and current data before retrying changes.",
    );
    await page.keyboard.press("Escape");
    await expect(page.getByText("raw-table", { exact: true })).toHaveCount(0);
    await expect(nav).toContainText("Raw workspace 原文");
    await expectWithinWidth(nav, 360);
  });
}
