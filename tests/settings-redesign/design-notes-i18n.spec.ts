const language = "ja";
import { switchLanguage } from "./i18n-support";
import { expect, test, type Page } from "@playwright/test";
test.beforeEach(async ({ page }) => {
  page.on("pageerror", (error) =>
    console.error("[fixture page error]", error.message),
  );
  page.on("requestfailed", (request) =>
    console.error(
      "[fixture request failed]",
      request.url(),
      request.failure()?.errorText,
    ),
  );
});
async function open(page: Page, language: string, extra = "") {
  await page.goto(`/?view=design-notes&lang=${language}${extra}`);
  await expect(page.getByTestId("design-notes")).toBeVisible({
    timeout: 15000,
  });
}
test(`design notes ${language}: unsaved title and body survive locale switching`, async ({
  page,
}) => {
  await open(page, language);
  await page.getByRole("button", { name: "原文 Title α" }).click();
  await page
    .getByRole("button", {
      name: "本文を編集",
      exact: true,
    })
    .click();
  await page
    .getByLabel("タイトル", { exact: true })
    .fill("Draft 原文 untouched");
  const body = page.getByRole("textbox", {
    name: "本文（Markdown）",
    exact: true,
  });
  await body.fill("Draft **Markdown** 原文");
  const other = "en";
  await switchLanguage(page, other);
  await expect(page.locator("html")).toHaveAttribute("lang", other);
  await page.getByRole("tab", { name: "Edit", exact: true }).click();
  await expect(page.getByLabel("Title", { exact: true })).toHaveValue(
    "Draft 原文 untouched",
  );
  await expect(
    page.getByRole("textbox", {
      name: "Body (Markdown)",
      exact: true,
    }),
  ).toHaveValue("Draft **Markdown** 原文");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(
    await page.evaluate(
      () => Reflect.get(window, "__designNoteFixture").writes,
    ),
  ).toEqual([]);
});
test(`design notes ${language}: conflict keeps drafts and captured revision`, async ({
  page,
}) => {
  await open(page, language);
  await page.getByRole("button", { name: "原文 Title α" }).click();
  await page
    .getByRole("button", {
      name: "本文を編集",
      exact: true,
    })
    .click();
  await page
    .getByLabel("タイトル", { exact: true })
    .fill("Preserved draft 原文");
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("入力は保持しています。");
  await expect(page.getByRole("alert")).not.toContainText("fixture private");
  await expect(page.getByLabel("タイトル", { exact: true })).toHaveValue(
    "Preserved draft 原文",
  );
  await expect(page.getByTestId("body-conflict-current")).toContainText(
    "Remote title 原文",
  );
  const writes = await page.evaluate(
    () =>
      (
        window as unknown as {
          __designNoteFixture: {
            writes: Array<{
              operation: {
                expected_revision: number;
              };
            }>;
          };
        }
      ).__designNoteFixture.writes,
  );
  expect(writes).toHaveLength(1);
  expect(writes[0].operation.expected_revision).toBe(1);
  await page
    .getByRole("button", {
      name: "現在版を確認したので本文の下書きを再適用",
      exact: true,
    })
    .click();
  await expect(page.getByLabel("タイトル", { exact: true })).toHaveValue(
    "Preserved draft 原文",
  );
  await page.getByRole("button", { name: "保存", exact: true }).click();
  const revisions = await page.evaluate(() =>
    (
      window as unknown as {
        __designNoteFixture: {
          writes: Array<{
            operation: {
              expected_revision: number;
            };
          }>;
        };
      }
    ).__designNoteFixture.writes.map((w) => w.operation.expected_revision),
  );
  expect(revisions).toEqual([1, 2]);
});
test(`design notes ${language}: read failure hides private details`, async ({
  page,
}) => {
  await open(page, language, "&notesState=error");
  await expect(page.getByRole("alert")).toHaveText(
    "設計メモを読み込めませんでした。",
  );
  await expect(page.getByRole("alert")).not.toContainText("fixture private");
});
