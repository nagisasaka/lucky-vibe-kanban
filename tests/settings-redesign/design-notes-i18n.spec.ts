const language = "ja";
import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
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
test(`design notes ${language}: metadata, raw content, Markdown drafts and locale switching`, async ({
  page,
}) => {
  await open(page, language);
  await expect(
    page.getByRole("searchbox", {
      name: "設計メモを検索",
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "原文 Title α" }).click();
  await expect(
    page.getByRole("heading", { name: "原文 Title α", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("note-body")).toContainText(
    "Original human body 日本語",
  );
  await expect(
    page.getByRole("heading", {
      name: "会話からの追記",
      exact: true,
    }),
  ).toBeVisible();
  const addition = page.getByTestId("note-addition");
  await expect(addition).toContainText("提案");
  for (const text of [
    "Original addition 日本語",
    "Original proposal 内容",
    "Speaker 原文",
    "Conversation #42 原文",
    "agent-recorder",
    "Original quote 原文",
  ])
    await expect(addition).toContainText(text);
  const expectedDate = await page.evaluate(
    (locale) => new Date("2026-01-02T15:04:05Z").toLocaleString(locale),
    language,
  );
  await expect(page.locator("header")).toContainText(expectedDate);
  await page
    .getByRole("button", {
      name: "履歴・初版",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("button", {
      name: new RegExp("初版"),
    }),
  ).toBeVisible();
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
  await page.getByRole("tab", { name: "プレビュー", exact: true }).click();
  await expect(
    page.getByTestId("note-body-editor").locator("strong"),
  ).toHaveText("Markdown");
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
  for (const width of [360, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await expectNoHorizontalOverflow(page);
  }
  await expect(page.getByTestId("design-notes")).not.toContainText(
    "designNotes.",
  );
});
test(`design notes ${language}: conflict keeps drafts and captured revision`, async ({
  page,
}) => {
  await open(page, language, "&notesSave=conflict");
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
