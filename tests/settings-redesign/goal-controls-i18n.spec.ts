import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";
for (const lang of ["en", "ja"]) {
  const ja = lang === "ja";
  test(`Goal controls ${lang}: exact usage, raw draft, confirmed clear and saved resume`, async ({
    page,
  }) => {
    await page.goto(`/?view=goal-controls&lang=${lang}`);
    await expect(
      page.getByText("Raw objective 日本語", { exact: true }),
    ).toBeVisible();
    await expect(page.getByText(/9,007,199,254,740,993/)).toHaveCount(2);
    await page
      .getByRole("button", {
        name: ja ? "Goalを編集" : "Edit Goal",
        exact: true,
      })
      .click();
    const draft = page.getByRole("textbox", {
      name: ja ? "Goalの目標" : "Goal objective",
      exact: true,
    });
    await draft.fill("  Raw revised 日本語  ");
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(
      page.getByRole("textbox", {
        name: ja ? "Goal objective" : "Goalの目標",
        exact: true,
      }),
    ).toHaveValue("  Raw revised 日本語  ");
    await page
      .getByRole("button", {
        name: ja ? "Save Goal" : "Goalを保存",
        exact: true,
      })
      .click();
    await expect(page.getByTestId("goal-control-calls")).toContainText(
      '"value":"Raw revised 日本語"',
    );
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toBe(
        ja ? "Clear this Goal permanently?" : "このGoalを完全に消去しますか？",
      );
      await dialog.dismiss();
    });
    await page
      .getByRole("button", {
        name: ja ? "Clear Goal" : "Goalを消去",
        exact: true,
      })
      .click();
    await expect(page.getByTestId("goal-control-calls")).not.toContainText(
      "clear",
    );
    page.once("dialog", async (dialog) => {
      await dialog.accept();
    });
    await page
      .getByRole("button", {
        name: ja ? "Clear Goal" : "Goalを消去",
        exact: true,
      })
      .click();
    await page
      .getByRole("button", {
        name: ja ? "Resume Goal" : "Goalを再開",
        exact: true,
      })
      .click();
    await expect(page.getByTestId("goal-control-calls")).toContainText(
      "resume-saved",
    );
    await expect(page.getByTestId("goal-control-calls")).toContainText("clear");
    await page.setViewportSize({ width: 360, height: 900 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  });
}
