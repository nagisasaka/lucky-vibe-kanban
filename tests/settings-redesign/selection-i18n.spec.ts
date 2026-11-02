import { expectWithinWidth, switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

for (const lang of ["en", "ja"]) {
  test(`Selection dialogs ${lang}: owned labels, raw identities, live locale and canonical results`, async ({
    page,
  }) => {
    let ja = lang === "ja";
    await page.goto(`/?view=action-labels&lang=${lang}`);
    const dialog = page.getByRole("dialog");
    const result = page.getByTestId("selection-result");
    await page.getByTestId("select-priority").click();
    await expect(dialog).toHaveAccessibleName(
      ja ? "コマンドメニュー" : "Command Menu",
    );
    await expect(dialog.getByRole("combobox")).toHaveAttribute(
      "placeholder",
      ja ? "優先度を選択" : "Select Priority",
    );
    await dialog.getByRole("combobox").fill("fixture-no-matches-831");
    await expect(dialog).toContainText(
      ja ? "結果が見つかりません。" : "No results found.",
    );
    await dialog.getByRole("combobox").fill(ja ? "緊急" : "Urgent");
    await expect(
      dialog.getByRole("option", { name: ja ? "緊急" : "Urgent", exact: true }),
    ).toBeVisible();
    await switchLanguage(page, ja ? "en" : "ja");
    ja = !ja;
    if (ja) {
      // The canonical priority ID remains a valid search term across languages.
      await expect(
        dialog.getByRole("option", { name: "緊急", exact: true }),
      ).toBeVisible();
    } else {
      await expect(dialog).toContainText("No results found.");
    }
    await dialog.getByRole("combobox").fill(ja ? "高" : "High");
    await dialog
      .getByRole("option", { name: ja ? "高" : "High", exact: true })
      .click();
    await expect(result).toHaveText('{"priority":"high"}');

    for (const [mode, title, raw, expected] of [
      [
        "status",
        ja ? "ステータスを選択" : "Select Status",
        "High",
        '{"statusId":"raw-status"}',
      ],
      [
        "repo",
        ja ? "リポジトリを選択" : "Select Repository",
        "High: raw repository",
        '{"repoId":"raw-repo"}',
      ],
      [
        "branch",
        ja
          ? "High: raw repository のブランチを選択"
          : "Select Branch for High: raw repository",
        "feature/High-raw-branch",
        '{"branch":"feature/High-raw-branch"}',
      ],
      [
        "parent",
        ja ? "親Issueを選択" : "Make Sub-issue of",
        "High: raw user issue",
        '{"type":"selected","issueId":"raw-issue"}',
      ],
      [
        "relationship",
        ja ? "Issueを選択" : "Select Issue",
        "High: raw user issue",
        '{"issueId":"raw-issue"}',
      ],
    ]) {
      await page.getByTestId(`select-${mode}`).click();
      await expect(dialog.getByRole("combobox")).toHaveAttribute(
        "placeholder",
        title,
      );
      await expect(dialog).toContainText(raw);
      await dialog.getByRole("combobox").fill(raw);
      await dialog.getByRole("option").click();
      await expect(result).toHaveText(expected);
    }
    await page.getByTestId("select-child").click();
    await expect(dialog.getByRole("combobox")).toHaveAttribute(
      "placeholder",
      ja ? "子Issueを追加" : "Add Sub-issue",
    );
    await dialog
      .getByRole("combobox")
      .fill(ja ? "新しい課題を作成" : "Create new issue");
    await expect(dialog.getByRole("option")).toHaveCount(1);
    await dialog.getByRole("option").click();
    await expect(result).toHaveText('{"type":"createNew"}');
    await page.getByTestId("select-repo").click();
    await page.setViewportSize({ width: 360, height: 800 });
    await expectWithinWidth(dialog, 360);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(result).toHaveText("cancelled");
  });
}
