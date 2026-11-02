const lang = "ja";
import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
test(`Agent Configuration ${lang}: sections, validation, native confirmation and locale switch`, async ({
  page,
}) => {
  await page.goto(`/?view=agent-configuration&lang=${lang}`);
  await expect(
    page.getByRole("heading", {
      name: "インストール済みエージェントの設定",
      exact: true,
    }),
  ).toBeVisible({ timeout: 15000 });
  await expect(
    page.getByRole("button", {
      name: "再読み込み",
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "一般", exact: true }).click();
  const reasoning = page.getByRole("combobox", {
    name: "推論の強度",
    exact: true,
  });
  await expect(
    reasoning.getByRole("option", { name: "高", exact: true }),
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
    page.getByText("2以上の値を入力してください。", { exact: true }),
  ).toBeVisible();
  await switchLanguage(page, "en");
  await expect(
    page.getByText("Minimum value is 2.", { exact: true }),
  ).toBeVisible();
  await switchLanguage(page, lang);
  await page
    .getByRole("tab", {
      name: "設定ファイル",
      exact: true,
    })
    .click();
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("認証情報");
    await dialog.dismiss();
  });
  await page.getByRole("button", { name: "直接編集", exact: true }).click();
  await expect(
    page.getByText("/fixture/config.toml", { exact: true }),
  ).toBeVisible();
  await expect(page.getByText(/解析済み/)).toBeVisible();
  await page.setViewportSize({ width: 360, height: 800 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
test(`Agent Configuration ${lang}: structured error language and safe unknown fallback`, async ({
  page,
}) => {
  await page.goto(
    `/?view=agent-configuration&lang=${lang}&configurationError=stale_revision`,
  );
  await expect(page.getByRole("alert")).toHaveText(
    "読み込み後に設定が変更されています。再読み込みし、差分を確認してから再試行してください。",
  );
  await switchLanguage(page, "en");
  await expect(page.getByRole("alert")).toHaveText(
    "The settings changed since they were loaded. Refresh and review the changes before retrying.",
  );
  await page.goto(
    `/?view=agent-configuration&lang=${lang}&configurationError=future_code`,
  );
  await expect(page.getByRole("alert")).toHaveText(
    "操作結果を確認できませんでした。設定を再読み込みして状態を確認してから再試行してください。",
  );
  await expect(page.locator("body")).not.toContainText("PRIVATE_");
});
test(`Agent Configuration ${lang}: sanitized diff explains preservation before confirm`, async ({
  page,
}) => {
  await page.goto(`/?view=agent-configuration&lang=${lang}`);
  await page.getByRole("tab", { name: "一般", exact: true }).click();
  await page.locator('[data-testid="agent-settings-panel"] input').fill("3");
  await page.getByRole("button", { name: "保存", exact: true }).click();
  const diff = page.getByRole("dialog", {
    name: "設定差分の確認",
    exact: true,
  });
  await expect(diff).toBeVisible();
  await expect(diff).toContainText("確認済みの設定リビジョン（値は非表示）");
  await expect(diff).toContainText(
    "指定した設定を適用します。変更対象外の設定値は保持します。",
  );
  await expect(diff).toContainText("実行中のセッションは再起動しません。");
  await expect(
    diff.getByRole("button", {
      name: "適用を確定",
      exact: true,
    }),
  ).toBeEnabled();
  await diff.getByRole("button", { name: "キャンセル", exact: true }).click();
  await expect(diff).not.toBeVisible();
  await expect(
    page.locator('[data-testid="agent-settings-panel"] input'),
  ).toHaveValue("3");
});
