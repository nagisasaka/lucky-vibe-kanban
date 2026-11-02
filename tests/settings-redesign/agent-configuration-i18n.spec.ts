import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
for (const lang of ["en", "ja"]) {
  const ja = lang === "ja";
  test(`Agent Configuration ${lang}: sections, validation, native confirmation and locale switch`, async ({
    page,
  }) => {
    await page.goto(`/?view=agent-configuration&lang=${lang}`);
    await expect(
      page.getByRole("heading", {
        name: ja
          ? "インストール済みエージェントの設定"
          : "Installed Agent settings",
        exact: true,
      }),
    ).toBeVisible({ timeout: 15000 });
    await expect(
      page.getByRole("button", {
        name: ja ? "再読み込み" : "Refresh",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByRole("tab", { name: ja ? "一般" : "General", exact: true })
      .click();
    const reasoning = page.getByRole("combobox", {
      name: ja ? "推論の強度" : "Reasoning effort",
      exact: true,
    });
    await expect(
      reasoning.getByRole("option", { name: ja ? "高" : "High", exact: true }),
    ).toHaveAttribute("value", '"high"');
    await expect(
      reasoning.getByRole("option", {
        name: "Raw future option 原文",
        exact: true,
      }),
    ).toHaveAttribute("value", '"future_option"');
    await reasoning.selectOption('"high"');
    await expect(reasoning).toHaveValue('"high"');
    await page.locator('[data-testid="agent-settings-panel"] input').fill("1");
    await expect(
      page.getByText(
        ja ? "2以上の値を入力してください。" : "Minimum value is 2.",
        { exact: true },
      ),
    ).toBeVisible();
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(
      page.getByText(
        ja ? "Minimum value is 2." : "2以上の値を入力してください。",
        { exact: true },
      ),
    ).toBeVisible();
    await switchLanguage(page, lang);
    await page
      .getByRole("tab", {
        name: ja ? "設定ファイル" : "Native Files",
        exact: true,
      })
      .click();
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain(ja ? "認証情報" : "credentials");
      await dialog.dismiss();
    });
    await page
      .getByRole("button", { name: ja ? "直接編集" : "Edit raw", exact: true })
      .click();
    await expect(
      page.getByText("/fixture/config.toml", { exact: true }),
    ).toBeVisible();
    await expect(page.getByText(ja ? /解析済み/ : /Parsed/)).toBeVisible();
    await page.setViewportSize({ width: 360, height: 800 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  });
}

for (const lang of ["en", "ja"]) {
  const ja = lang === "ja";
  test(`Agent Configuration ${lang}: structured error language and safe unknown fallback`, async ({
    page,
  }) => {
    await page.goto(
      `/?view=agent-configuration&lang=${lang}&configurationError=stale_revision`,
    );
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "読み込み後に設定が変更されています。再読み込みし、差分を確認してから再試行してください。"
        : "The settings changed since they were loaded. Refresh and review the changes before retrying.",
    );
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "The settings changed since they were loaded. Refresh and review the changes before retrying."
        : "読み込み後に設定が変更されています。再読み込みし、差分を確認してから再試行してください。",
    );
    await page.goto(
      `/?view=agent-configuration&lang=${lang}&configurationError=future_code`,
    );
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "操作結果を確認できませんでした。設定を再読み込みして状態を確認してから再試行してください。"
        : "The operation result could not be confirmed. Refresh and check the settings before retrying.",
    );
    await expect(page.locator("body")).not.toContainText("PRIVATE_");
  });
}

for (const lang of ["en", "ja"]) {
  const ja = lang === "ja";
  test(`Agent Configuration ${lang}: sanitized diff explains preservation before confirm`, async ({
    page,
  }) => {
    await page.goto(`/?view=agent-configuration&lang=${lang}`);
    await page
      .getByRole("tab", { name: ja ? "一般" : "General", exact: true })
      .click();
    await page.locator('[data-testid="agent-settings-panel"] input').fill("3");
    await page
      .getByRole("button", { name: ja ? "保存" : "Save", exact: true })
      .click();
    const diff = page.getByRole("dialog", {
      name: ja ? "設定差分の確認" : "Settings diff confirmation",
      exact: true,
    });
    await expect(diff).toBeVisible();
    await expect(diff).toContainText(
      ja
        ? "確認済みの設定リビジョン（値は非表示）"
        : "Observed native revision (values withheld)",
    );
    await expect(diff).toContainText(
      ja
        ? "指定した設定を適用します。変更対象外の設定値は保持します。"
        : "Requested settings will be applied; unrelated native values are preserved.",
    );
    await expect(diff).toContainText(
      ja
        ? "実行中のセッションは再起動しません。"
        : "active sessions are not restarted.",
    );
    await expect(
      diff.getByRole("button", {
        name: ja ? "適用を確定" : "Confirm apply",
        exact: true,
      }),
    ).toBeEnabled();
    await diff
      .getByRole("button", { name: ja ? "キャンセル" : "Cancel", exact: true })
      .click();
    await expect(diff).not.toBeVisible();
    await expect(
      page.locator('[data-testid="agent-settings-panel"] input'),
    ).toHaveValue("3");
  });
}
