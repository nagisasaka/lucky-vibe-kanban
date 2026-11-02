import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
const lang = "ja";
test(`notifications ${lang}: live sentences preserve grouped read targets and links`, async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.clock.setFixedTime(new Date("2026-11-01T12:03:00Z"));
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto(`/?view=notifications&lang=${lang}`);
  const bell = page.getByRole("button", {
    name: "通知",
    exact: true,
  });
  await expect(bell).toHaveText("2", { timeout: 15000 });
  await expect(page.locator('[aria-label="ユーザー"]')).toBeVisible();
  const rows = page.locator('[role="button"]');
  await expect(rows).toHaveCount(2);
  const comments = rows.filter({ hasText: "RAW-12" });
  await expect(comments).toContainText("2件のコメントを追加しました");
  await expect(comments.locator('[aria-label="Raw Actor 原文"]')).toBeVisible();
  await expect(comments).toContainText("3分前");
  await switchLanguage(page, "en");
  await expect(page.locator('[aria-label="User"]')).toBeVisible();
  await expect(comments).toContainText("left 2 comments on");
  await expect(rows.filter({ hasText: "RAW-99" })).toContainText(
    "Someone changed the priority of RAW-99 from Low to High",
  );
  await expect(comments).toContainText("3m ago");
  await expectWithinWidth(comments, 360);
  await comments
    .getByRole("button", {
      name: "Mark notification as read",
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
      name: "Mark all as read",
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
      name: "Notifications",
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
test(`notifications ${lang}: sign-in gate, sync error and unconfirmed mutation follow locale`, async ({
  page,
}) => {
  await page.goto(`/?view=notifications&lang=${lang}&notifications-disabled`);
  await expect(
    page.getByText("通知を表示するにはログインしてください", { exact: true }),
  ).toBeVisible({ timeout: 15000 });
  await expect(page.getByRole("button")).toHaveCount(0);
  await page.goto(`/?view=notifications&lang=${lang}&notifications-error`);
  await expect(page.getByRole("alert")).toContainText(
    "通知を読み込めませんでした。",
  );
  await expect(page.locator("body")).not.toContainText("Raw internal secret");
  await page.getByRole("button", { name: "再試行", exact: true }).click();
  await expect
    .poll(
      async () =>
        JSON.parse(
          (await page.getByTestId("notification-evidence").textContent())!,
        ).retries,
    )
    .toBe(1);
  await page.goto(`/?view=notifications&lang=${lang}&notifications-mark-error`);
  await page
    .getByRole("button", {
      name: "この通知を既読にする",
      exact: true,
    })
    .first()
    .click();
  await expect(page.getByRole("alert")).toHaveText(
    "既読への変更結果を確認できませんでした。再試行する前に現在の状態を確認してください。",
  );
  await expect(page.locator("body")).not.toContainText("Raw mutation secret");
  await switchLanguage(page, "en");
  await expect(page.getByRole("alert")).toHaveText(
    "Marking notifications as read could not be confirmed. Check their current state before retrying.",
  );
});
