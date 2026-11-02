import { switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
test(`target branch JA→EN: selection survives live locale and unknown results stay safe`, async ({
  page,
}) => {
  await page.goto(`/?view=repository-dialogs&lang=ja`);
  await page.getByRole("button", { name: "Change fixture target" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox").click();
  const option = page.getByRole("option", {
    name: "feature/raw-target-原文 (現在)",
    exact: true,
  });
  await option.click();
  await switchLanguage(page, "en");
  await expect(dialog.getByRole("combobox")).toContainText(
    "feature/raw-target-原文",
  );
  await expect(page.getByTestId("repo-events")).toHaveText("[]");
  await dialog
    .getByRole("button", {
      name: "Change Branch",
      exact: true,
    })
    .click();
  await expect(dialog.getByRole("alert")).toHaveText(
    "The target branch change could not be confirmed. Check the current branch before retrying.",
  );
  await expect(dialog).not.toContainText("Raw target secret");
  await switchLanguage(page, "ja");
  await expect(dialog.getByRole("alert")).toHaveText(
    "ターゲットブランチの変更結果を確認できませんでした。再試行する前に現在のブランチを確認してください。",
  );
  await expect(dialog.getByRole("combobox")).toContainText(
    "feature/raw-target-原文",
  );
  await expect(page.getByTestId("repo-events")).toHaveText(
    '[{"kind":"branch","branch":"feature/raw-target-原文"}]',
  );
  await dialog.getByRole("button", { name: "キャンセル", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId("repo-events")).toHaveText(
    '[{"kind":"branch","branch":"feature/raw-target-原文"}]',
  );
});
for (const outcome of ["throw", "auth"]) {
  test(`PR ${outcome}: safe live notice preserves draft and sends once`, async ({
    page,
  }) => {
    const pageErrors: string[] = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    await page.goto(`/?view=repository-dialogs&lang=ja&pr=${outcome}`);
    await page
      .getByRole("button", { name: "Create fixture PR", exact: true })
      .click();
    // KeyboardDialog renders a portal without a dialog role.
    const dialog = page.locator("body");
    const title = dialog.getByLabel("タイトル", { exact: true });
    await expect(title).toHaveValue("Raw initial title 原文 (vibe-kanban)");
    await dialog
      .getByRole("checkbox", {
        name: "AIエージェントにより良いPR説明を生成させる",
      })
      .uncheck();
    const rawTitle = "Raw title {{token}} 原文";
    const rawBody = "Raw body {{title}}\n原文";
    await title.fill(rawTitle);
    await dialog.getByLabel("説明 (オプション)", { exact: true }).fill(rawBody);
    await dialog.getByRole("button", { name: "PRを作成", exact: true }).click();
    await expect(dialog).toContainText(
      outcome === "throw"
        ? "PRの作成結果を確認できません"
        : "GitHub認証を確認できません",
    );
    await expect(
      dialog.getByRole("button", { name: "PRを作成", exact: true }),
    ).toBeEnabled();
    await expect(dialog).not.toContainText("Raw PR secret");
    await switchLanguage(page, "en");
    await expect(dialog).toContainText(
      outcome === "throw"
        ? "PR creation could not be confirmed."
        : "GitHub authentication could not be confirmed.",
    );
    await expect(dialog.getByLabel("Title", { exact: true })).toHaveValue(
      rawTitle,
    );
    await expect(
      dialog.getByLabel("Description (optional)", { exact: true }),
    ).toHaveValue(rawBody);
    const calls = () =>
      page.evaluate(() => Reflect.get(window, "__prFixtureCalls"));
    expect(await calls()).toEqual([
      {
        path: "/api/workspaces/raw-pr-workspace/pull-requests",
        body: {
          title: rawTitle,
          body: rawBody,
          target_branch: "feature/raw-target-原文",
          draft: false,
          auto_generate_description: false,
          repo_id: "raw-repo",
        },
      },
    ]);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByLabel("Title", { exact: true })).toHaveCount(0);
    await expect(page.getByTestId("repo-events")).toContainText(
      '"success":false,"error":',
    );
    expect(await calls()).toHaveLength(1);
    expect(pageErrors).toEqual([]);
  });
}
