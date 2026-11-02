import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
test(`Executor schema JA→EN: generated copy, numeric validation and live language`, async ({
  page,
}) => {
  await page.goto(`/?view=schema-form&lang=ja`);
  await expect(
    page.getByText("追加のプロンプト", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("プロンプトの末尾に追加するテキスト", { exact: true }),
  ).toBeVisible();
  await page.locator("#root_sandbox").click();
  await expect(
    page.getByRole("menuitem", {
      name: "設定ファイルに従う",
      exact: true,
    }),
  ).toBeVisible();
  await page
    .getByRole("menuitem", {
      name: "ワークスペースへの書き込み",
      exact: true,
    })
    .click();
  await expect(page.getByTestId("schema-raw-values")).toContainText(
    '"sandbox":"workspace-write"',
  );
  await expect(
    page.getByRole("textbox", { name: "モデル", exact: true }),
  ).toHaveValue("Raw model 原文");
  await page.getByRole("button", { name: /詳細設定/ }).click();
  const budget = page.locator("#root_goal_token_budget");
  await budget.fill("-1");
  await expect(
    page.getByText("0以上の値を入力してください。", { exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("schema-raw-values")).toContainText(
    '"goal_token_budget":-1',
  );
  await switchLanguage(page, "en");
  await expect(
    page.getByText("Goal Token Budget", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("Enter a value of at least 0.", { exact: true }),
  ).toBeVisible();
  await expect(budget).toHaveValue("-1");
  const save = page.getByRole("button", {
    name: "Save",
    exact: true,
  });
  await expect(save).toBeDisabled();
  await page.getByRole("button", { name: /Advanced configuration/ }).click();
  await expect(save).toBeDisabled();
  await page.getByRole("button", { name: /Advanced configuration/ }).click();
  await budget.fill("2");
  await expect(save).toBeEnabled();
  await page.setViewportSize({ width: 360, height: 800 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await expect(page.locator("#root_model")).toHaveValue("Raw model 原文");
});
