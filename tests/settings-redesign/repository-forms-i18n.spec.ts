import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
import { expect, test, type Page } from "@playwright/test";

for (const language of ["en", "ja"] as const) {
  const ja = language === "ja";
  test(`configuration ${language}: validation, clone identifiers and live locale changes`, async ({
    page,
  }) => {
    await page.goto(`/?view=repository-forms&lang=${language}`);
    await page
      .getByRole("button", {
        name: "Fixture create configuration",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("dialog", {
        name: ja ? "新しい設定を作成" : "Create New Configuration",
        exact: true,
      }),
    ).toBeVisible();
    const name = page.getByLabel(ja ? "設定名" : "Configuration Name", {
      exact: true,
    });
    await name.fill("日本語");
    await page
      .getByRole("button", {
        name: ja ? "設定を作成" : "Create Configuration",
        exact: true,
      })
      .click();
    await expect(page.getByRole("alert")).toContainText(
      ja ? "設定名には半角英数字" : "Configuration name can only contain",
    );
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(page.getByRole("alert")).toContainText(
      ja ? "Configuration name can only contain" : "設定名には半角英数字",
    );
    await switchLanguage(page, language);
    await name.fill("DEFAULT");
    await page
      .getByRole("button", {
        name: ja ? "設定を作成" : "Create Configuration",
        exact: true,
      })
      .click();
    await expect(page.getByRole("alert")).toContainText(
      ja
        ? "同じ名前の設定がすでに存在します。"
        : "A configuration with this name already exists",
    );
    await name.fill("NEW_PROFILE");
    await page
      .getByRole("combobox", {
        name: ja ? "コピー元（任意）" : "Clone from (optional)",
        exact: true,
      })
      .click();
    await page
      .getByRole("option", {
        name: ja ? "DEFAULT からコピー" : "Clone from DEFAULT",
        exact: true,
      })
      .click();
    await page
      .getByRole("button", {
        name: ja ? "設定を作成" : "Create Configuration",
        exact: true,
      })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(
      JSON.parse(await page.getByTestId("configuration-result").innerText()),
    ).toEqual({
      action: "created",
      configName: "NEW_PROFILE",
      cloneFrom: "DEFAULT",
    });
    await page
      .getByRole("button", {
        name: "Fixture delete configuration",
        exact: true,
      })
      .click();
    await expect(page.getByRole("dialog")).toContainText("CUSTOM_RAW");
    await page
      .getByRole("button", { name: ja ? "キャンセル" : "Cancel", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });

  test(`environment field ${language}: translated controls preserve raw keys and values`, async ({
    page,
  }) => {
    await page.goto(`/?view=repository-forms&lang=${language}`);
    const field = page.getByTestId("environment-field");
    await expect(
      field.getByLabel(ja ? "環境変数名" : "Environment variable key", {
        exact: true,
      }),
    ).toHaveValue("RAW_KEY");
    await field
      .getByLabel(ja ? "RAW_KEYの値" : "Value for RAW_KEY", { exact: true })
      .fill("Raw updated value 原文");
    await field
      .getByLabel(ja ? "追加する環境変数名" : "New environment variable key", {
        exact: true,
      })
      .fill("EXTRA_RAW");
    await field
      .getByLabel(
        ja ? "追加する環境変数の値" : "New environment variable value",
        { exact: true },
      )
      .fill("Extra value 日本語");
    await field
      .getByRole("button", {
        name: ja ? "環境変数を追加" : "Add environment variable",
        exact: true,
      })
      .click();
    expect(
      JSON.parse(await field.getByTestId("environment-value").innerText()),
    ).toEqual({
      RAW_KEY: "Raw updated value 原文",
      EXTRA_RAW: "Extra value 日本語",
    });
    await switchLanguage(page, ja ? "en" : "ja");
    await expect(
      field.getByLabel(ja ? "Value for EXTRA_RAW" : "EXTRA_RAWの値", {
        exact: true,
      }),
    ).toHaveValue("Extra value 日本語");
    await field
      .getByRole("button", {
        name: ja ? "Remove RAW_KEY" : "RAW_KEYを削除",
        exact: true,
      })
      .click();
    expect(
      JSON.parse(await field.getByTestId("environment-value").innerText()),
    ).toEqual({ EXTRA_RAW: "Extra value 日本語" });
  });

  test(`repository memory ${language}: UI locale does not change output language or start maintenance`, async ({
    page,
  }) => {
    await page.goto(`/?view=repository-forms&lang=${language}`);
    const card = page.getByTestId("repository-memory");
    await expect(card).toContainText(ja ? "状態: 最新" : "Status: Up to date");
    const timestamp = await page.evaluate(
      (locale) => new Date("2026-01-02T15:04:05Z").toLocaleString(locale),
      language,
    );
    await expect(card).toContainText(timestamp);
    await expect(
      card.getByLabel(ja ? "出力言語（BCP 47）" : "Output language (BCP 47)", {
        exact: true,
      }),
    ).toHaveValue("en");
    await card
      .getByLabel(ja ? "統合先のローカルブランチ" : "Integrated local branch", {
        exact: true,
      })
      .fill("release/raw");
    await expect(
      card.getByRole("button", {
        name: ja ? "Wikiを同期" : "Sync Wiki",
        exact: true,
      }),
    ).toBeDisabled();
    await card
      .getByRole("button", {
        name: ja ? "設定を保存" : "Save settings",
        exact: true,
      })
      .click();
    const other = ja ? "en" : "ja";
    await switchLanguage(page, other);
    await expect(
      card.getByLabel(ja ? "Output language (BCP 47)" : "出力言語（BCP 47）", {
        exact: true,
      }),
    ).toHaveValue("en");
    const calls = await page.evaluate(
      () =>
        (
          window as unknown as {
            __repositoryFormsCalls: Array<{ action: string; data: unknown }>;
          }
        ).__repositoryFormsCalls,
    );
    expect(calls).toEqual([
      {
        action: "configureMemory",
        data: {
          enabled: true,
          target_branch: "release/raw",
          output_language: "en",
        },
      },
    ]);
    await card
      .getByLabel(ja ? "Reconciliation scope" : "同期の対象範囲", {
        exact: true,
      })
      .click();
    await page
      .getByRole("menuitem", {
        name: ja ? "Changes since a commit" : "指定コミットより後の変更",
        exact: true,
      })
      .click();
    await card
      .getByLabel(ja ? "Starting commit" : "開始コミット", { exact: true })
      .fill("not-a-sha");
    await expect(
      card.getByRole("button", {
        name: ja ? "Sync Wiki" : "Wikiを同期",
        exact: true,
      }),
    ).toBeDisabled();
    await expect(
      card.getByLabel(ja ? "Starting commit" : "開始コミット", { exact: true }),
    ).toHaveAttribute("aria-invalid", "true");
    await expect(card).toContainText(
      ja
        ? "Enter a 4–40 character hexadecimal commit SHA from this branch."
        : "このブランチ上のコミットSHAを、4〜40桁の16進数で入力してください。",
    );
    await page.setViewportSize({ width: 360, height: 900 });
    await expectNoHorizontalOverflow(page);
  });

  test(`repository memory ${language}: unknown error details stay out of normal UI`, async ({
    page,
  }) => {
    await page.goto(
      `/?view=repository-forms&lang=${language}&memoryError=true`,
    );
    const memory = page.getByTestId("repository-memory");
    await expect(memory).toContainText("11111111-1111-1111-1111-111111111111");
    await expect(memory).toContainText("22222222-2222-2222-2222-222222222222");
    await expect(page.locator("body")).not.toContainText("fixture private");
  });
}
