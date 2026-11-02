import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
for (const lang of ["en", "ja"]) {
  test(`notifications ${lang}: live sentences preserve grouped read targets and links`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.clock.setFixedTime(new Date("2026-11-01T12:03:00Z"));
    await page.setViewportSize({ width: 360, height: 800 });
    await page.goto(`/?view=notifications&lang=${lang}`);
    const bell = page.getByRole("button", {
      name: lang === "ja" ? "通知" : "Notifications",
      exact: true,
    });
    await expect(bell).toHaveText("2", { timeout: 15000 });
    await expect(
      page.locator(`[aria-label="${lang === "ja" ? "ユーザー" : "User"}"]`),
    ).toBeVisible();
    const rows = page.locator('[role="button"]');
    await expect(rows).toHaveCount(2);
    const comments = rows.filter({ hasText: "RAW-12" });
    await expect(comments).toContainText(
      lang === "ja" ? "2件のコメントを追加しました" : "left 2 comments on",
    );
    await expect(
      comments.locator('[aria-label="Raw Actor 原文"]'),
    ).toBeVisible();
    await expect(comments).toContainText(lang === "ja" ? "3分前" : "3m ago");
    await switchLanguage(page, lang === "ja" ? "en" : "ja");
    const ja = lang !== "ja";
    await expect(
      page.locator(`[aria-label="${ja ? "ユーザー" : "User"}"]`),
    ).toBeVisible();
    await expect(comments).toContainText(
      ja ? "2件のコメントを追加しました" : "left 2 comments on",
    );
    await expect(rows.filter({ hasText: "RAW-99" })).toContainText(
      ja
        ? "誰かがRAW-99の優先度を「低」から「高」に変更しました"
        : "Someone changed the priority of RAW-99 from Low to High",
    );
    await expect(comments).toContainText(ja ? "3分前" : "3m ago");
    await expectWithinWidth(comments, 360);
    await comments
      .getByRole("button", {
        name: ja ? "この通知を既読にする" : "Mark notification as read",
        exact: true,
      })
      .click();
    const evidence = async () =>
      JSON.parse(
        (await page.getByTestId("notification-evidence").textContent())!,
      );
    await expect
      .poll(async () => (await evidence()).writes)
      .toEqual([
        [
          { id: "n2", changes: { seen: true } },
          { id: "n1", changes: { seen: true } },
        ],
      ]);
    expect((await evidence()).navigations).toEqual([]);
    await page
      .getByRole("button", {
        name: ja ? "すべて既読にする" : "Mark all as read",
        exact: true,
      })
      .click();
    await expect
      .poll(async () => (await evidence()).writes)
      .toEqual([
        [
          { id: "n2", changes: { seen: true } },
          { id: "n1", changes: { seen: true } },
        ],
        [{ id: "n3", changes: { seen: true } }],
      ]);
    await expect(
      page.getByRole("button", {
        name: ja ? "通知" : "Notifications",
        exact: true,
      }),
    ).toHaveText("");
    await rows.filter({ hasText: "RAW-99" }).press("Enter");
    await expect
      .poll(async () => (await evidence()).navigations)
      .toEqual([{ to: "/raw/issue-99" }]);
    expect((await evidence()).writes).toHaveLength(2);
    expect(errors).toEqual([]);
  });
  test(`notifications ${lang}: separate sign-in/loading/empty/sync and unconfirmed mutation states`, async ({
    page,
  }) => {
    const ja = lang === "ja";
    await page.goto(`/?view=notifications&lang=${lang}&notifications-disabled`);
    await expect(
      page.getByText(
        ja
          ? "通知を表示するにはログインしてください"
          : "Sign in to view notifications",
        { exact: true },
      ),
    ).toBeVisible({ timeout: 15000 });
    await expect(page.getByRole("button")).toHaveCount(0);
    await page.goto(`/?view=notifications&lang=${lang}&notifications-loading`);
    await expect(
      page.getByText(ja ? "通知を読み込み中…" : "Loading notifications…", {
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByText(ja ? "通知はまだありません" : "No notifications yet", {
        exact: true,
      }),
    ).toHaveCount(0);
    await page.goto(`/?view=notifications&lang=${lang}&notifications-empty`);
    await expect(
      page.getByText(ja ? "通知はまだありません" : "No notifications yet", {
        exact: true,
      }),
    ).toBeVisible();
    await page.goto(`/?view=notifications&lang=${lang}&notifications-error`);
    await expect(page.getByRole("alert")).toContainText(
      ja
        ? "通知を読み込めませんでした。"
        : "Notifications could not be loaded.",
    );
    await expect(page.locator("body")).not.toContainText("Raw internal secret");
    await page
      .getByRole("button", { name: ja ? "再試行" : "Retry", exact: true })
      .click();
    await expect
      .poll(
        async () =>
          JSON.parse(
            (await page.getByTestId("notification-evidence").textContent())!,
          ).retries,
      )
      .toBe(1);
    await page.goto(
      `/?view=notifications&lang=${lang}&notifications-mark-error`,
    );
    await page
      .getByRole("button", {
        name: ja ? "この通知を既読にする" : "Mark notification as read",
        exact: true,
      })
      .first()
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "既読への変更結果を確認できませんでした。再試行する前に現在の状態を確認してください。"
        : "Marking notifications as read could not be confirmed. Check their current state before retrying.",
    );
    await expect(page.locator("body")).not.toContainText("Raw mutation secret");
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "Marking notifications as read could not be confirmed. Check their current state before retrying."
        : "既読への変更結果を確認できませんでした。再試行する前に現在の状態を確認してください。",
    );
  });
}
