import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Files JA→EN: lazy raw paths, copy bytes and passive workspace controls", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const paths: string[] = [];
    Object.defineProperty(window, "__copiedPaths", { value: paths });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          paths.push(text);
        },
      },
    });
  });
  await page.goto("/?view=file-viewer&lang=ja&file-usage=execution_only");
  const preview = page.getByTestId("file-preview");
  await expect(preview).toContainText("ファイルを選択", { timeout: 15000 });
  await page.getByRole("button", { name: "Raw 原文.txt", exact: true }).click();
  await expect(preview).toContainText("Raw file {{user}} 原文");
  await expect(preview).toContainText("テキスト");
  await expect(preview).toContainText("1.5 KB");
  await page
    .getByRole("textbox", { name: "ファイルを検索", exact: true })
    .fill("Raw 原文");
  const target = await page.getByTestId("file-target").textContent();
  await switchLanguage(page, "en");
  await expect(
    page.getByRole("textbox", { name: "Search files", exact: true }),
  ).toHaveValue("Raw 原文");
  await expect(page.getByTestId("file-target")).toHaveText(target!);
  await expect(preview).toContainText("Text");
  await expect(
    preview.getByRole("button", { name: "Open in editor", exact: true }),
  ).toHaveCount(0);
  await preview.getByRole("button", { name: "Copy path", exact: true }).click();
  await expect(
    preview.getByRole("button", { name: "Copied path", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { __copiedPaths: string[] }).__copiedPaths,
    ),
  ).toEqual(["Raw 原文.txt"]);
  await page.getByRole("textbox").fill("");
  await page.getByRole("button", { name: "src", exact: true }).click();
  await page
    .getByRole("button", { name: "child 原文.txt", exact: true })
    .click();
  expect(
    JSON.parse((await page.getByTestId("file-target").textContent())!),
  ).toEqual({
    workspaceId: "raw-file-workspace",
    repoId: "raw-repo",
    path: "src/child 原文.txt",
    source: "file-tree",
    sessionId: "raw-session",
  });
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: async () => {
        throw new Error("Denied fixture");
      },
    }),
  );
  await preview.getByRole("button", { name: "Copy path", exact: true }).click();
  await expect(
    preview.getByRole("button", {
      name: "Copy requested. Check the clipboard result.",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    preview.getByRole("button", { name: "Copied path", exact: true }),
  ).toHaveCount(0);
});

test("File preview failure JA→EN withholds private bodies and editor actions", async ({
  page,
}) => {
  await page.goto(
    "/?view=file-viewer&lang=ja&files-mode=preview-error&file-usage=execution_only",
  );
  await page.getByRole("button", { name: "Raw 原文.txt", exact: true }).click();
  await expect(
    page.getByText("プレビューを読み込めませんでした", { exact: true }),
  ).toBeVisible();
  await switchLanguage(page, "en");
  await expect(
    page.getByText("Could not load preview", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open in editor", exact: true }),
  ).toHaveCount(0);
  await expect(page.locator("body")).not.toContainText("Raw internal");
});
