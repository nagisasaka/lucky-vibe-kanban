import { expectNoHorizontalOverflow } from "./i18n-support";
import { expect, test } from "@playwright/test";

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`owned authentication screens use ${language} before and after sign-in`, async ({
    page,
  }) => {
    await page.goto(`/?auth=locked&lang=${language}`);
    await expect(page.locator("html")).toHaveAttribute("lang", language);
    await expect(
      page.getByRole("heading", {
        name: ja ? "LVK ログイン・初期設定" : "LVK Sign in and initial setup",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByLabel(ja ? "セットアップキー" : "Setup key", { exact: true }),
    ).toHaveAttribute("type", "password");
    await page.reload();
    await expect(
      page.getByRole("button", {
        name: ja ? "設定を開始" : "Start setup",
        exact: true,
      }),
    ).toBeVisible();
    await page.goto(`/?auth=locked&access=recovery&lang=${language}`);
    await expect(
      page.getByLabel(ja ? "復旧キー" : "Recovery key", { exact: true }),
    ).toBeVisible();

    await page.evaluate(() =>
      sessionStorage.setItem("settings-fixture-logged-out", "true"),
    );
    await page.goto(`/?auth=basic&lang=${language}`);
    await page
      .getByLabel(ja ? "ユーザー名" : "Username", { exact: true })
      .fill("sample-only-user");
    await page
      .getByLabel(ja ? "パスワード" : "Password", { exact: true })
      .fill("sample-only-password");
    await page
      .getByRole("button", { name: ja ? "ログイン" : "Sign in", exact: true })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "この操作を行う権限がありません。"
        : "You do not have permission for this operation.",
    );
    await expect(page.getByRole("alert")).not.toContainText("Fixture:");
    await page.goto(`/?auth=auth0&lang=${language}`);
    await expect(
      page.getByRole("button", {
        name: ja ? "Auth0でログイン" : "Sign in with Auth0",
        exact: true,
      }),
    ).toBeVisible();

    await page.evaluate(() => sessionStorage.clear());
    await page.goto(`/?access=settings&auth=auth0&lang=${language}`);
    await expect(
      page.getByRole("heading", {
        name: ja ? "LVK 認証とメンバー管理" : "LVK Authentication and members",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByText(
        ja ? "Auth0を自動セットアップ" : "Set up Auth0 automatically",
        { exact: true },
      )
      .click();
    await expect(
      page.getByLabel(
        ja ? "Management APIアクセストークン" : "Management API access token",
        { exact: true },
      ),
    ).toHaveAttribute("type", "password");
    await expect(
      page.getByRole("button", {
        name: ja ? "接続とアプリを取得" : "Load connections and apps",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: ja ? "無効化" : "Disable",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByText(
        ja
          ? "既存の接続情報を保存・更新"
          : "Save or update existing credentials",
        { exact: true },
      )
      .click();
    await expect(
      page.getByLabel(
        ja ? "保存するChatGPT Client Secret" : "ChatGPT Client Secret to save",
        { exact: true },
      ),
    ).toHaveAttribute("type", "password");
    const copyButton = page
      .getByRole("button", { name: ja ? "コピー" : "Copy", exact: true })
      .first();
    await page.evaluate(() =>
      Object.defineProperty(navigator.clipboard, "writeText", {
        configurable: true,
        value: async () => {},
      }),
    );
    await copyButton.click();
    await expect(page.getByRole("status")).toHaveText(
      ja ? "コピーしました" : "Copied",
    );
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
      ja
        ? "コピーできませんでした。入力欄の値を選択してコピーしてください。"
        : "Unable to copy. Select and copy the value from the field.",
    );
    await expect(page.getByRole("alert")).not.toContainText("fixture private");
    for (const width of [360, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      await expectNoHorizontalOverflow(page);
    }
    await expect(page.locator("body")).not.toContainText(
      /auth\.(gate|setup|credentials|errors)\./,
    );
  });
}
