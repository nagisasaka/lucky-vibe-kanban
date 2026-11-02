import { switchLanguage } from "./i18n-support";
import { expect, test } from "@playwright/test";

test("Onboarding preserves the unsaved draft and saves one exact scoped patch", async ({
  page,
}) => {
  await page.goto("/?view=onboarding&lang=ja");
  await page.getByRole("button", { name: "Codex", exact: true }).click();
  await page.getByRole("button", { name: "音なし", exact: true }).click();
  await page
    .getByRole("textbox", { name: "メニュー表示名", exact: true })
    .fill("Raw editor draft 原文");
  await switchLanguage(page, "en");
  await expect(
    page.getByRole("textbox", { name: "Menu name", exact: true }),
  ).toHaveValue("Raw editor draft 原文");
  const writes = () =>
    page.evaluate(() => Reflect.get(window, "__onboardingWrites"));
  expect(await writes()).toEqual([]);
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Saving...", exact: true }),
  ).toBeDisabled();
  expect(await writes()).toHaveLength(1);
  expect((await writes())[0]).toMatchObject({
    onboarding_acknowledged: true,
    disclaimer_acknowledged: true,
    executor_profile: { executor: "CODEX", variant: null },
    editor: {
      entries: [
        { name: "Raw editor draft 原文", uri_template: "vscode://file/{path}" },
      ],
    },
    notifications: { sound_enabled: false, push_enabled: false },
  });
  expect((await writes())[0]).not.toHaveProperty("language");
  await page.evaluate(() => Reflect.get(window, "__finishOnboarding")());
  await expect
    .poll(() =>
      page.evaluate(
        () => Reflect.get(window, "__onboardingNavigations").length,
      ),
    )
    .toBeGreaterThan(0);
});

test("Onboarding unconfirmed save withholds provider details", async ({
  page,
}) => {
  await page.goto("/?view=onboarding&lang=ja&onboarding-error");
  await page.getByRole("button", { name: "続行", exact: true }).click();
  await page.evaluate(() => Reflect.get(window, "__finishOnboarding")());
  await expect(page.getByRole("alert")).toHaveText(
    "保存結果を確認できませんでした。再試行する前に現在の設定を確認してください。",
  );
  await expect(page.locator("body")).not.toContainText("Raw provider secret");
});
