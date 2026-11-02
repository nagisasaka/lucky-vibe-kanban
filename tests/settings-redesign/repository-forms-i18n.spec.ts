const language = "ja";
import { expectNoHorizontalOverflow, switchLanguage } from "./i18n-support";
import { expect, test, type Page } from "@playwright/test";
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
      name: "新しい設定を作成",
      exact: true,
    }),
  ).toBeVisible();
  const name = page.getByLabel("設定名", {
    exact: true,
  });
  await name.fill("日本語");
  await page
    .getByRole("button", {
      name: "設定を作成",
      exact: true,
    })
    .click();
  await expect(page.getByRole("alert")).toContainText("設定名には半角英数字");
  await switchLanguage(page, "en");
  await expect(page.getByRole("alert")).toContainText(
    "Configuration name can only contain",
  );
  await switchLanguage(page, language);
  await name.fill("DEFAULT");
  await page
    .getByRole("button", {
      name: "設定を作成",
      exact: true,
    })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "同じ名前の設定がすでに存在します。",
  );
  await name.fill("NEW_PROFILE");
  await page
    .getByRole("combobox", {
      name: "コピー元（任意）",
      exact: true,
    })
    .click();
  await page
    .getByRole("option", {
      name: "DEFAULT からコピー",
      exact: true,
    })
    .click();
  await page
    .getByRole("button", {
      name: "設定を作成",
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
  await page.getByRole("button", { name: "キャンセル", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
test(`environment field ${language}: translated controls preserve raw keys and values`, async ({
  page,
}) => {
  await page.goto(`/?view=repository-forms&lang=${language}`);
  const field = page.getByTestId("environment-field");
  await expect(
    field.getByLabel("環境変数名", {
      exact: true,
    }),
  ).toHaveValue("RAW_KEY");
  await field
    .getByLabel("RAW_KEYの値", { exact: true })
    .fill("Raw updated value 原文");
  await field
    .getByLabel("追加する環境変数名", {
      exact: true,
    })
    .fill("EXTRA_RAW");
  await field
    .getByLabel("追加する環境変数の値", { exact: true })
    .fill("Extra value 日本語");
  await field
    .getByRole("button", {
      name: "環境変数を追加",
      exact: true,
    })
    .click();
  expect(
    JSON.parse(await field.getByTestId("environment-value").innerText()),
  ).toEqual({
    RAW_KEY: "Raw updated value 原文",
    EXTRA_RAW: "Extra value 日本語",
  });
  await switchLanguage(page, "en");
  await expect(
    field.getByLabel("Value for EXTRA_RAW", {
      exact: true,
    }),
  ).toHaveValue("Extra value 日本語");
  await field
    .getByRole("button", {
      name: "Remove RAW_KEY",
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
  await expect(card).toContainText("状態: 最新");
  const timestamp = await page.evaluate(
    (locale) => new Date("2026-01-02T15:04:05Z").toLocaleString(locale),
    language,
  );
  await expect(card).toContainText(timestamp);
  await expect(
    card.getByLabel("出力言語（BCP 47）", {
      exact: true,
    }),
  ).toHaveValue("en");
  await card
    .getByLabel("統合先のローカルブランチ", {
      exact: true,
    })
    .fill("release/raw");
  await expect(
    card.getByRole("button", {
      name: "Wikiを同期",
      exact: true,
    }),
  ).toBeDisabled();
  await card
    .getByRole("button", {
      name: "設定を保存",
      exact: true,
    })
    .click();
  const other = "en";
  await switchLanguage(page, other);
  await expect(
    card.getByLabel("Output language (BCP 47)", {
      exact: true,
    }),
  ).toHaveValue("en");
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          __repositoryFormsCalls: Array<{
            action: string;
            data: unknown;
          }>;
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
    .getByLabel("Reconciliation scope", {
      exact: true,
    })
    .click();
  await page
    .getByRole("menuitem", {
      name: "Changes since a commit",
      exact: true,
    })
    .click();
  await card.getByLabel("Starting commit", { exact: true }).fill("not-a-sha");
  await expect(
    card.getByRole("button", {
      name: "Sync Wiki",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(
    card.getByLabel("Starting commit", { exact: true }),
  ).toHaveAttribute("aria-invalid", "true");
  await expect(card).toContainText(
    "Enter a 4–40 character hexadecimal commit SHA from this branch.",
  );
  await page.setViewportSize({ width: 360, height: 900 });
  await expectNoHorizontalOverflow(page);
});
test(`repository memory ${language}: unknown error details stay out of normal UI`, async ({
  page,
}) => {
  await page.goto(`/?view=repository-forms&lang=${language}&memoryError=true`);
  const memory = page.getByTestId("repository-memory");
  await expect(memory).toContainText("11111111-1111-1111-1111-111111111111");
  await expect(memory).toContainText("22222222-2222-2222-2222-222222222222");
  await expect(page.locator("body")).not.toContainText("fixture private");
});
