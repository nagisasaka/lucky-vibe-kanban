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
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "設定", exact: true });
      await dialog
        .getByRole("navigation")
        .getByRole("button", { name: "アカウント", exact: true })
        .click();
      await expect(
        dialog.getByRole("heading", { name: "ログイン状態", exact: true }),
      ).toBeVisible();
      await expect(
        dialog.getByText(auth === "local" ? "Local接続" : "ログイン済み", {
          exact: true,
        }),
      ).toBeVisible();
      await expect(
        dialog.getByText(
          auth === "auth0" ? "Auth0" : auth === "basic" ? "Basic" : "Local",
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
        await page.evaluate(async () => {
          await document.fonts.ready;
          await Promise.all(
            document
              .getAnimations()
              .filter(
                (a) => a.effect?.getComputedTiming().iterations !== Infinity,
              )
              .map((a) => a.finished.catch(() => undefined)),
          );
        });
        await dialog.screenshot({
          path: `.build/settings-account-review/account-${auth}-${width}.png`,
          animations: "disabled",
        });
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
      page.getByRole("heading", { name: "ログイン状態", exact: true }),
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
test("logout failure stays in account with error; English account labels", async ({
  page,
}) => {
  await page.goto(
    "/?section=account&lang=en&allowLogout=true&logoutFailure=true",
  );
  await page.getByRole("button", { name: "設定を開く", exact: true }).click();
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "Account", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Log out", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText("Fixture logout failed");
  await expect(page.getByText("Signed in", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Log out", exact: true }),
  ).toBeEnabled();
});
