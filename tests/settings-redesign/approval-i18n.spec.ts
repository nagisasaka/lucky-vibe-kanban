import {
  expectWithinWidth,
  switchLanguage as changeLanguage,
} from "./i18n-support";
import { test, expect, type Page } from "@playwright/test";
const calls = (page: Page) =>
  page.evaluate(
    () => (window as unknown as { __approvalCalls: unknown[] }).__approvalCalls,
  );

for (const language of ["en", "ja"]) {
  test(`approval ${language}: keyboard scopes, submitting and exact approval target`, async ({
    page,
  }) => {
    const ja = language === "ja";
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`/?view=approval&lang=${language}&approval-hold`);
    const approve = () =>
      page.getByRole("button", { name: ja ? "承認" : "Approve", exact: true });
    await expect(approve()).toBeEnabled({ timeout: 15000 });
    await expect(page.getByTestId("approval-scopes")).toContainText(
      "approvals",
    );
    await page
      .getByRole("button", { name: "Toggle fixture dialog scope" })
      .click();
    await page.evaluate(() => (document.activeElement as HTMLElement)?.blur());
    await page.keyboard.press("Enter");
    expect(await calls(page)).toEqual([]);
    await page
      .getByRole("button", { name: "Toggle fixture dialog scope" })
      .click();
    await page.getByRole("button", { name: "Toggle fixture tab" }).click();
    await page.evaluate(() => (document.activeElement as HTMLElement)?.blur());
    await page.keyboard.press("Enter");
    expect(await calls(page)).toEqual([]);
    await page.getByRole("button", { name: "Toggle fixture tab" }).click();
    await expect(page.getByTestId("approval-scopes")).toContainText(
      "approvals",
    );
    await page.locator("body").dispatchEvent("keydown", {
      key: "Enter",
      code: "Enter",
      isComposing: true,
    });
    expect(await calls(page)).toEqual([]);
    await page.evaluate(() => (document.activeElement as HTMLElement)?.blur());
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("button", {
        name: ja ? "承認を送信中" : "Submitting approval",
        exact: true,
      }),
    ).toBeDisabled();
    await page.keyboard.press("Enter");
    expect(await calls(page)).toEqual([
      { execution_process_id: "raw-process", status: { status: "approved" } },
    ]);
    await page.evaluate(() =>
      (
        window as unknown as { __finishApproval: () => void }
      ).__finishApproval(),
    );
    await expect(approve()).toBeDisabled();
    await expect(
      page.getByRole("button", { name: ja ? "拒否" : "Deny", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByText("Raw tool 原文 / raw-command --flag", { exact: true }),
    ).toBeVisible();
    expect(errors).toEqual([]);
  });
  test(`approval ${language}: denial draft follows language without rewriting input or fallback payload`, async ({
    page,
  }) => {
    let ja = language === "ja";
    await page.setViewportSize({ width: 360, height: 800 });
    await page.goto(`/?view=approval&lang=${language}`);
    await page
      .getByRole("button", { name: ja ? "拒否" : "Deny", exact: true })
      .click();
    const editor = () =>
      page.getByRole("textbox", {
        name: ja ? "Markdownエディター" : "Markdown editor",
        exact: true,
      });
    await editor().fill("Raw denial 原文");
    await changeLanguage(page, ja ? "en" : "ja");
    ja = !ja;
    await expect(editor()).toHaveText("Raw denial 原文");
    await expect(
      page.getByText(
        ja
          ? "このリクエストを拒否する理由をエージェントに伝えてください。@でタグの挿入やファイルの検索ができます。"
          : "Let the agent know why this request was denied... Type @ to insert tags or search files.",
        { exact: true },
      ),
    ).toHaveCount(0);
    await page
      .getByRole("button", { name: ja ? "キャンセル" : "Cancel", exact: true })
      .click();
    expect(await calls(page)).toEqual([]);
    await page
      .getByRole("button", { name: ja ? "拒否" : "Deny", exact: true })
      .click();
    await expect(editor()).toBeEmpty();
    const placeholder = page.getByText(
      ja
        ? "このリクエストを拒否する理由をエージェントに伝えてください。@でタグの挿入やファイルの検索ができます。"
        : "Let the agent know why this request was denied... Type @ to insert tags or search files.",
      { exact: true },
    );
    await expectWithinWidth(placeholder, 360);
    await editor().fill("Raw denial 原文");
    await editor().press("Control+Enter");
    await expect
      .poll(() => calls(page))
      .toEqual([
        {
          execution_process_id: "raw-process",
          status: { status: "denied", reason: "Raw denial 原文" },
        },
      ]);
    await page.goto(`/?view=approval&lang=${language}`);
    ja = language === "ja";
    await page
      .getByRole("button", { name: ja ? "拒否" : "Deny", exact: true })
      .click();
    await page
      .getByRole("button", { name: ja ? "拒否" : "Deny", exact: true })
      .click();
    await expect
      .poll(() => calls(page))
      .toEqual([
        {
          execution_process_id: "raw-process",
          status: {
            status: "denied",
            reason: "User denied this tool use request.",
          },
        },
      ]);
  });
  test(`approval ${language}: safe response errors, missing context and expired request`, async ({
    page,
  }) => {
    const ja = language === "ja";
    await page.goto(`/?view=approval&lang=${language}&approval-error`);
    await page
      .getByRole("button", { name: ja ? "承認" : "Approve", exact: true })
      .click();
    await expect(page.getByRole("alert")).toHaveText(
      ja
        ? "応答結果を確認できませんでした。再試行する前にリクエストと現在のセッションを確認してください。"
        : "Your response could not be confirmed. Check the request and current session before retrying.",
    );
    await expect(page.locator("body")).not.toContainText(
      "Raw approval provider secret",
    );
    await changeLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toContainText(
      ja
        ? "Your response could not be confirmed"
        : "応答結果を確認できませんでした",
    );
    await page.goto(`/?view=approval&lang=${language}&approval-missing`);
    await page
      .getByRole("button", { name: ja ? "承認" : "Approve", exact: true })
      .click();
    await expect(page.getByRole("alert")).toContainText(
      ja
        ? "実行情報がありません"
        : "execution context for this request is missing",
    );
    expect(await calls(page)).toEqual([]);
    await page.goto(`/?view=approval&lang=${language}&approval-expired`);
    await expect(
      page.getByRole("button", { name: ja ? "承認" : "Approve", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: ja ? "拒否" : "Deny", exact: true }),
    ).toBeDisabled();
    await page.keyboard.press("Enter");
    expect(await calls(page)).toEqual([]);
  });
  test(`markdown controls ${language}: requested copy keeps bytes and raw source while labels switch`, async ({
    page,
  }) => {
    const ja = language === "ja";
    await page.addInitScript(() => {
      const writes: string[] = [];
      Object.defineProperty(window, "__markdownWrites", { value: writes });
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            writes.push(text);
            throw Error("Fixture denied clipboard");
          },
        },
      });
    });
    await page.goto(`/?view=approval&lang=${language}`);
    const preview = page.getByTestId("markdown-copy");
    await preview.hover();
    await preview
      .getByRole("button", {
        name: ja ? "Markdownとしてコピー" : "Copy as Markdown",
        exact: true,
      })
      .click();
    await expect(
      preview.getByRole("button", {
        name: ja
          ? "コピーを依頼しました。クリップボードの内容を確認してください。"
          : "Copy requested. Check the clipboard result.",
        exact: true,
      }),
    ).toBeVisible();
    await changeLanguage(page, ja ? "en" : "ja");
    await expect(
      preview.getByRole("button", {
        name: ja
          ? "Copy requested. Check the clipboard result."
          : "コピーを依頼しました。クリップボードの内容を確認してください。",
        exact: true,
      }),
    ).toBeVisible();
    await preview.hover();
    await preview
      .getByRole("button", { name: ja ? "Edit" : "編集", exact: true })
      .click();
    await preview.hover();
    await preview
      .getByRole("button", { name: ja ? "Delete" : "削除", exact: true })
      .click();
    await expect(page.getByTestId("editor-events")).toHaveText(
      '["edit","delete"]',
    );
    const source = page.getByTestId("markdown-source");
    await source.hover();
    await source
      .getByRole("button", {
        name: ja ? "Copy as Markdown" : "Markdownとしてコピー",
        exact: true,
      })
      .click();
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __markdownWrites: string[] })
            .__markdownWrites,
      ),
    ).toEqual(["Raw_value 原文", "Raw\\_source 原文"]);
  });
}
