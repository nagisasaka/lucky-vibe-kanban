import { expect, test } from "@playwright/test";

for (const width of [360, 1440]) {
  for (const auth of ["local", "basic", "auth0"]) {
    test(`account ${auth} ${width}: existing settings entry, session and logout`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: width === 360 ? 844 : 1040 });
      await page.goto(`/?auth=${auth}&allowLogout=true&gear=true`);
      // No independent floating settings/logout actions outside the dialog.
      await expect(
        page.getByRole("button", { name: "ログアウト", exact: true }),
      ).toHaveCount(0);
      await page.getByRole("button", { name: "設定", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "設定", exact: true });
      await dialog
        .getByRole("navigation")
        .getByRole("button", { name: "アカウント", exact: true })
        .click();
      await expect(
        dialog.getByRole("heading", { name: "接続情報", exact: true }),
      ).toBeVisible();
      await expect(
        dialog.getByText(
          auth === "local"
            ? "Local接続"
            : auth === "basic"
              ? "sample-basic-user"
              : "Sample Auth0 Person",
          {
            exact: true,
          },
        ),
      ).toBeVisible();
      await expect(
        dialog.getByText(
          auth === "auth0" ? "Auth0" : auth === "basic" ? "Basic" : "ローカル",
          { exact: true },
        ),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      if (auth === "local") {
        await expect(
          dialog.getByRole("button", { name: "ログアウト", exact: true }),
        ).toHaveCount(0);
      } else {
        await Promise.all([
          page.waitForURL((url) => url.search === ""),
          dialog
            .getByRole("button", { name: "ログアウト", exact: true })
            .click(),
        ]);
        expect(
          await page.evaluate(() =>
            sessionStorage.getItem("settings-fixture-logged-out"),
          ),
        ).toBe("true");
        await expect(
          page.getByRole("heading", {
            name: "LVK ログイン・初期設定",
            exact: true,
          }),
        ).toBeVisible();
      }
    });
  }
}
for (const auth of ["basic", "auth0"]) {
  test(`member ${auth}: account and logout available without management APIs`, async ({
    page,
  }) => {
    await page.goto(
      `/?auth=${auth}&role=member&section=account&allowLogout=true`,
    );
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "接続情報", exact: true }),
    ).toBeVisible();
    await expect(
      page
        .getByRole("navigation")
        .getByRole("button", { name: "認証・メンバー", exact: true }),
    ).toHaveCount(0);
    expect(
      await page.evaluate(() =>
        (
          window as unknown as { __settingsFixtureCalls: string[] }
        ).__settingsFixtureCalls.filter((p) =>
          /\/api\/access\/(configuration|members|audit)/.test(p),
        ),
      ),
    ).toEqual([]);
    await page.getByRole("button", { name: "ログアウト", exact: true }).click();
    await expect(
      page.getByRole("heading", {
        name: "LVK ログイン・初期設定",
        exact: true,
      }),
    ).toBeVisible();
  });
}
for (const language of ["en", "ja"]) {
  test(`logout failure stays in account with a safe ${language} error`, async ({
    page,
  }) => {
    await page.goto(
      `/?section=account&lang=${language}&allowLogout=true&logoutFailure=true`,
    );
    await page.getByRole("button", { name: "設定を開く", exact: true }).click();
    await expect(
      page.getByRole("navigation").getByRole("button", {
        name: language === "ja" ? "アカウント" : "Account",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: language === "ja" ? "ログアウト" : "Log out",
        exact: true,
      })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      language === "ja"
        ? "認証リクエストを完了できませんでした。サーバーの稼働状態と現在の認証設定を確認してください。"
        : "The authentication request could not be completed. Check the server status and current authentication settings.",
    );
    await expect(page.getByRole("alert")).not.toContainText(
      "Fixture logout failed",
    );
    await expect(
      page.getByText("sample-basic-user", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: language === "ja" ? "ログアウト" : "Log out",
        exact: true,
      }),
    ).toBeEnabled();
  });
}

for (const auth of ["basic", "auth0"]) {
  for (const identity of ["missing", "blank"]) {
    test(`account ${auth} ${identity}: do not invent identity`, async ({
      page,
    }) => {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`/?auth=${auth}&identity=${identity}&gear=true`);
      await page.getByRole("button", { name: "設定", exact: true }).click();
      await page
        .getByRole("navigation")
        .getByRole("button", { name: "アカウント", exact: true })
        .click();
      await expect(
        page.getByText("本人情報を取得できません", { exact: true }),
      ).toBeVisible();
      await expect(page.getByText("ログイン済み", { exact: true })).toHaveCount(
        0,
      );
      await expect(page.getByText("fixture-user", { exact: true })).toHaveCount(
        0,
      );
      await expect(
        page.getByRole("button", { name: "ログアウト", exact: true }),
      ).toBeVisible();
    });
  }
}
