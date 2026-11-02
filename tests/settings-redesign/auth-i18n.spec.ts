const language = "ja";
import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
test(`owned authentication screens use ${language} before and after sign-in`, async ({
  page,
}) => {
  await page.goto(`/?auth=locked&lang=${language}`);
  await expect(page.locator("html")).toHaveAttribute("lang", language);
  await expect(
    page.getByRole("heading", {
      name: "LVK ログイン・初期設定",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByLabel("セットアップキー", { exact: true }),
  ).toHaveAttribute("type", "password");
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: "設定を開始",
      exact: true,
    }),
  ).toBeVisible();
  await page.goto(`/?auth=locked&access=recovery&lang=${language}`);
  await expect(page.getByLabel("復旧キー", { exact: true })).toBeVisible();
  await page.evaluate(() =>
    sessionStorage.setItem("settings-fixture-logged-out", "true"),
  );
  await page.goto(`/?auth=basic&lang=${language}`);
  await page.getByLabel("ユーザー名", { exact: true }).fill("sample-only-user");
  await page
    .getByLabel("パスワード", { exact: true })
    .fill("sample-only-password");
  await page.getByRole("button", { name: "ログイン", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "この操作を行う権限がありません。",
  );
  await expect(page.getByRole("alert")).not.toContainText("Fixture:");
  await page.goto(`/?auth=auth0&lang=${language}`);
  await expect(
    page.getByRole("button", {
      name: "Auth0でログイン",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => sessionStorage.clear());
  await page.goto(`/?access=settings&auth=auth0&lang=${language}`);
  await expect(
    page.getByRole("heading", {
      name: "LVK 認証とメンバー管理",
      exact: true,
    }),
  ).toBeVisible();
  await page.getByText("Auth0を自動セットアップ", { exact: true }).click();
  await expect(
    page.getByLabel("Management APIアクセストークン", { exact: true }),
  ).toHaveAttribute("type", "password");
  await expect(
    page.getByRole("button", {
      name: "接続とアプリを取得",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", {
      name: "無効化",
      exact: true,
    }),
  ).toBeVisible();
  await page.getByText("既存の接続情報を保存・更新", { exact: true }).click();
  await expect(
    page.getByLabel("保存するChatGPT Client Secret", { exact: true }),
  ).toHaveAttribute("type", "password");
  const copyButton = page
    .getByRole("button", { name: "コピー", exact: true })
    .first();
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: async () => {},
    }),
  );
  await copyButton.click();
  await expect(page.getByRole("status")).toHaveText("コピーしました");
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: async () => {
        throw new Error("fixture private clipboard detail");
      },
    }),
  );
  await copyButton.click();
  await expect(page.getByRole("alert")).toHaveText(
    "コピーできませんでした。入力欄の値を選択してコピーしてください。",
  );
  await expect(page.getByRole("alert")).not.toContainText("fixture private");
  await switchLanguage(page, "en");
  await expect(page.getByRole("alert")).toHaveText(
    "Unable to copy. Select and copy the value from the field.",
  );

  for (const width of [360, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await expectNoHorizontalOverflow(page);
  }
  await expect(page.locator("body")).not.toContainText(
    /auth\.(gate|setup|credentials|errors)\./,
  );
});
