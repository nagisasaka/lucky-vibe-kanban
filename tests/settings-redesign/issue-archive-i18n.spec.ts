import { switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
const calls = (page: import("@playwright/test").Page) =>
  page.evaluate(
    () =>
      (window as unknown as { __issueArchiveCalls: Record<string, unknown>[] })
        .__issueArchiveCalls,
  );
for (const lang of ["en", "ja"]) {
  test(`issue archive ${lang}: defaults, scoped targets and lost-response receipt recovery`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));

    await page.goto(`/?view=issue-archive&lang=${lang}&archive-lost-response`);
    await page.getByRole("button", { name: "Open archive fixture" }).click();
    const dialog = page.getByRole("dialog");
    const include = dialog.getByRole("checkbox");
    await expect(include).toBeChecked();
    const archive = () =>
      dialog.getByRole("button", {
        name: ja ? "アーカイブ" : "Archive",
        exact: true,
      });
    let ja = lang === "ja";
    await expect(archive()).toBeEnabled();
    await expect(dialog).toContainText("/raw/path 原文");
    await expect(dialog).toContainText("raw/branch");
    await expect(dialog).toContainText(ja ? "物理削除" : "physically deleted");
    await include.uncheck();
    await expect(archive()).toBeEnabled();
    await expect(dialog).toContainText(
      ja ? "課題のみアーカイブ" : "Only the issue will be archived",
    );
    await switchLanguage(page, ja ? "en" : "ja");
    ja = !ja;
    await expect(dialog.getByRole("heading")).toHaveText(
      ja ? "課題をアーカイブ" : "Archive issue",
    );
    await expect(include).not.toBeChecked();
    expect((await calls(page)).filter((c) => c.kind === "prepare")).toEqual([
      {
        kind: "prepare",
        projectId: "raw-project",
        issueId: "raw-issue",
        archiveWorkspaces: true,
      },
      {
        kind: "prepare",
        projectId: "raw-project",
        issueId: "raw-issue",
        archiveWorkspaces: false,
      },
    ]);

    await archive().click();
    await expect(dialog.getByRole("alert")).toContainText(
      ja ? "再実行せず「結果を確認」" : "Do not repeat the operation",
    );
    await expect(dialog).not.toContainText("Raw confirmation secret");
    await expect(include).toBeDisabled();
    const confirms = (await calls(page)).filter((c) => c.kind === "confirm");
    expect(confirms).toHaveLength(1);
    expect(confirms[0]).toMatchObject({
      projectId: "raw-project",
      issueId: "raw-issue",
      token: "token-off",
    });
    const requestId = confirms[0].requestId;
    await expect(dialog.getByRole("alert")).toContainText(requestId as string);

    await dialog
      .getByRole("button", {
        name: ja ? "結果を確認" : "Check result",
        exact: true,
      })
      .click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByTestId("archive-result")).toHaveText("true");
    expect(
      (await calls(page)).filter((c) => c.kind === "confirm"),
    ).toHaveLength(1);
    expect((await calls(page)).filter((c) => c.kind === "receipt")).toEqual([
      { kind: "receipt", requestId },
    ]);
    await page.getByRole("button", { name: "Open archive fixture" }).click();
    await expect(dialog.getByRole("checkbox")).toBeChecked();
    await dialog
      .getByRole("button", { name: ja ? "キャンセル" : "Cancel", exact: true })
      .click();
    await expect(page.getByTestId("archive-result")).toHaveText("false");
    expect(
      (await calls(page)).filter((c) => c.kind === "confirm"),
    ).toHaveLength(1);
    expect(errors).toEqual([]);
  });
  test(`issue archive ${lang}: warnings and read failures never expose raw bodies or enable repeat confirm`, async ({
    page,
  }) => {
    const ja = lang === "ja";
    await page.goto(`/?view=issue-archive&lang=${lang}&archive-warning`);
    await page.getByRole("button", { name: "Open archive fixture" }).click();
    let dialog = page.getByRole("dialog");
    await dialog
      .getByRole("button", { name: ja ? "アーカイブ" : "Archive", exact: true })
      .click();
    await expect(dialog.getByRole("alert")).toContainText(
      ja
        ? "ワークツリー削除を確認できませんでした"
        : "worktree deletion could not be confirmed",
    );
    await expect(dialog.getByRole("alert")).toContainText("Raw workspace 原文");
    await expect(dialog).not.toContainText("Raw deletion secret");
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(dialog.getByRole("alert")).toContainText(
      ja
        ? "worktree deletion could not be confirmed"
        : "ワークツリー削除を確認できませんでした",
    );
    await dialog
      .getByRole("button", { name: ja ? "Close" : "閉じる", exact: true })
      .last()
      .click();
    await expect(page.getByTestId("archive-result")).toHaveText("true");
    await page.goto(`/?view=issue-archive&lang=${lang}&archive-prepare-error`);
    await page.getByRole("button", { name: "Open archive fixture" }).click();
    dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("alert")).toContainText(
      ja
        ? "アーカイブ対象を確認できませんでした"
        : "archive targets could not be prepared",
    );
    await expect(
      dialog.getByRole("button", {
        name: ja ? "アーカイブ" : "Archive",
        exact: true,
      }),
    ).toBeDisabled();
    await expect(dialog).not.toContainText("Raw prepare secret");
    expect(
      (await calls(page)).filter((c) => c.kind === "confirm"),
    ).toHaveLength(0);
    await page.goto(
      `/?view=issue-archive&lang=${lang}&archive-lost-response&archive-inspect-error`,
    );
    await page.getByRole("button", { name: "Open archive fixture" }).click();
    dialog = page.getByRole("dialog");
    await dialog
      .getByRole("button", { name: ja ? "アーカイブ" : "Archive", exact: true })
      .click();
    const inspect = dialog.getByRole("button", {
      name: ja ? "結果を確認" : "Check result",
      exact: true,
    });
    await inspect.click();
    await expect(dialog.getByRole("alert")).toContainText(
      ja ? "結果を取得できませんでした" : "result could not be retrieved",
    );
    await expect(dialog).not.toContainText("Raw inspection secret");
    await expect(inspect).toBeEnabled();
    await expect(
      dialog.getByRole("button", {
        name: ja ? "アーカイブ" : "Archive",
        exact: true,
      }),
    ).toHaveCount(0);
    expect(
      (await calls(page)).filter((c) => c.kind === "confirm"),
    ).toHaveLength(1);
    await dialog
      .getByRole("button", { name: ja ? "キャンセル" : "Cancel", exact: true })
      .click();
  });
}

for (const language of ["en", "ja"]) {
  test(`unarchive action ${language}: transport/receipt identity survives shared error presentation`, async ({
    page,
  }) => {
    for (const mode of ["restore-lost-response", "restore-uncertain"]) {
      await page.goto(`/?view=issue-archive&lang=${language}&${mode}`);
      await page
        .getByRole("button", { name: "Restore fixture", exact: true })
        .click();
      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible();
      const read = () =>
        page.evaluate(
          () =>
            (
              window as unknown as {
                __archiveRestoreCalls: {
                  project_id: string;
                  request_id: string;
                }[];
              }
            ).__archiveRestoreCalls,
        );
      const requests = await read();
      expect(requests).toHaveLength(1);
      expect(requests[0].project_id).toBe("raw-project");
      expect(requests[0].request_id).toMatch(/^[0-9a-f-]{36}$/);
      await expect(dialog).toContainText(requests[0].request_id);
      await expect(dialog).not.toContainText("Raw restore transport secret");
      await expect(dialog).not.toContainText("Raw receipt secret");
      const ja = language === "ja";
      await expect(dialog).toContainText(
        mode === "restore-lost-response"
          ? ja
            ? "アーカイブ解除の結果を確認できませんでした"
            : "Unarchiving could not be confirmed"
          : ja
            ? "操作の完了を確認できていません"
            : "operation is not confirmed complete",
      );
      await switchLanguage(page, ja ? "en" : "ja");
      await expect(dialog).toContainText(
        mode === "restore-lost-response"
          ? ja
            ? "Unarchiving could not be confirmed"
            : "アーカイブ解除の結果を確認できませんでした"
          : ja
            ? "operation is not confirmed complete"
            : "操作の完了を確認できていません",
      );
      await expect(dialog).toContainText(requests[0].request_id);
      expect(await read()).toEqual(requests);
      await expect(page.getByTestId("archive-result")).toHaveText("null");
    }
  });
}
