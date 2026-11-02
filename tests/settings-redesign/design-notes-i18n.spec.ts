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

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`design notes ${language}: metadata, raw content, Markdown drafts and locale switching`, async ({
    page,
  }) => {
    await open(page, language);
    await expect(
      page.getByRole("searchbox", {
        name: ja ? "設計メモを検索" : "Search design notes",
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
        name: ja ? "会話からの追記" : "Additions from conversations",
        exact: true,
      }),
    ).toBeVisible();
    const addition = page.getByTestId("note-addition");
    await expect(addition).toContainText(ja ? "提案" : "Proposal");
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
        name: ja ? "履歴・初版" : "History and first version",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("button", {
        name: new RegExp(ja ? "初版" : "First version"),
      }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: ja ? "本文を編集" : "Edit body",
        exact: true,
      })
      .click();
    await page
      .getByLabel(ja ? "タイトル" : "Title", { exact: true })
      .fill("Draft 原文 untouched");
    const body = page.getByRole("textbox", {
      name: ja ? "本文（Markdown）" : "Body (Markdown)",
      exact: true,
    });
    await body.fill("Draft **Markdown** 原文");
    await page
      .getByRole("tab", { name: ja ? "プレビュー" : "Preview", exact: true })
      .click();
    await expect(
      page.getByTestId("note-body-editor").locator("strong"),
    ).toHaveText("Markdown");
    const other = ja ? "en" : "ja";
    await switchLanguage(page, other);
    await expect(page.locator("html")).toHaveAttribute("lang", other);
    await page
      .getByRole("tab", { name: ja ? "Edit" : "編集", exact: true })
      .click();
    await expect(
      page.getByLabel(ja ? "Title" : "タイトル", { exact: true }),
    ).toHaveValue("Draft 原文 untouched");
    await expect(
      page.getByRole("textbox", {
        name: ja ? "Body (Markdown)" : "本文（Markdown）",
        exact: true,
      }),
    ).toHaveValue("Draft **Markdown** 原文");
    await page
      .getByRole("button", { name: ja ? "Cancel" : "キャンセル", exact: true })
      .click();
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
        name: ja ? "本文を編集" : "Edit body",
        exact: true,
      })
      .click();
    await page
      .getByLabel(ja ? "タイトル" : "Title", { exact: true })
      .fill("Preserved draft 原文");
    await page
      .getByRole("button", { name: ja ? "保存" : "Save", exact: true })
      .click();
    await expect(page.getByRole("alert")).toContainText(
      ja ? "入力は保持しています。" : "Your input is preserved.",
    );
    await expect(page.getByRole("alert")).not.toContainText("fixture private");
    await expect(
      page.getByLabel(ja ? "タイトル" : "Title", { exact: true }),
    ).toHaveValue("Preserved draft 原文");
    await expect(page.getByTestId("body-conflict-current")).toContainText(
      "Remote title 原文",
    );
    const writes = await page.evaluate(
      () =>
        (
          window as unknown as {
            __designNoteFixture: {
              writes: Array<{ operation: { expected_revision: number } }>;
            };
          }
        ).__designNoteFixture.writes,
    );
    expect(writes).toHaveLength(1);
    expect(writes[0].operation.expected_revision).toBe(1);
    await page
      .getByRole("button", {
        name: ja
          ? "現在版を確認したので本文の下書きを再適用"
          : "Reapply body draft after reviewing the current version",
        exact: true,
      })
      .click();
    await expect(
      page.getByLabel(ja ? "タイトル" : "Title", { exact: true }),
    ).toHaveValue("Preserved draft 原文");
    await page
      .getByRole("button", { name: ja ? "保存" : "Save", exact: true })
      .click();
    const revisions = await page.evaluate(() =>
      (
        window as unknown as {
          __designNoteFixture: {
            writes: Array<{ operation: { expected_revision: number } }>;
          };
        }
      ).__designNoteFixture.writes.map((w) => w.operation.expected_revision),
    );
    expect(revisions).toEqual([1, 2]);
  });

  test(`design notes ${language}: empty, loading and read failure`, async ({
    page,
  }) => {
    await open(page, language, "&notesState=empty");
    await expect(
      page.getByText(
        ja
          ? "最初の考えと、その後の判断を残せます。"
          : "Keep the initial idea and the decisions that follow.",
        { exact: true },
      ),
    ).toBeVisible();
    await open(page, language, "&notesState=loading");
    await expect(
      page.getByText(ja ? "読み込み中…" : "Loading…", { exact: true }),
    ).toBeVisible();
    await open(page, language, "&notesState=error");
    await expect(page.getByRole("alert")).toHaveText(
      ja ? "設計メモを読み込めませんでした。" : "Unable to load design notes.",
    );
    await expect(page.getByRole("alert")).not.toContainText("fixture private");
  });
}
