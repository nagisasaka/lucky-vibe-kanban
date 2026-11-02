import { switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
test(`model selector JA→EN: live labels retain provider/model/reasoning and override semantics`, async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));

  await page.goto(`/?view=model-selector&lang=ja`);
  const controls = page.getByTestId("model-controls");
  await controls
    .getByRole("button", {
      name: "Raw Alpha 原文 · Raw reasoning 原文",
      exact: true,
    })
    .click({ timeout: 15000 });
  const menu = page.locator("[data-model-selector-popover]");

  const search = menu.getByPlaceholder("名前またはIDで絞り込む…");
  await search.fill("RAW:MODEL");
  await expect(
    menu.getByText("Raw provider A 原文", { exact: true }),
  ).toBeVisible();
  await switchLanguage(page, "en");
  const updatedSearch = menu.getByPlaceholder("Filter by name or ID...");
  await expect(updatedSearch).toHaveValue("RAW:MODEL");
  await updatedSearch.fill("raw:model");
  await menu
    .getByRole("button", { name: "Raw provider B 原文", exact: true })
    .click();
  await expect(
    menu.locator('[data-model-key="beta/raw:model"]'),
  ).toHaveAttribute("data-provider-id", "beta");
  await menu
    .getByRole("button", { name: "Raw Beta 原文", exact: true })
    .click();
  await expect(page.getByTestId("model-config")).toContainText(
    '"model_id":"beta/raw:model"',
  );
  await expect(page.getByTestId("model-config")).toContainText(
    '"reasoning_id":"raw-effort"',
  );
  await menu
    .getByRole("button", { name: "Raw reasoning 原文", exact: true })
    .click();
  await page
    .getByRole("menuitem", {
      name: "Follow CLI config",
      exact: true,
    })
    .click();
  await expect(page.getByTestId("model-config")).toContainText(
    '"reasoning_id":null',
  );
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await controls.getByRole("button", { name: "Code", exact: true }).click();
  await page.getByRole("menuitem", { name: "Goal", exact: true }).click();
  await controls
    .getByRole("button", {
      name: "Goal settings",
      exact: true,
    })
    .click();
  await page
    .getByRole("spinbutton", {
      name: "Token budget",
      exact: true,
    })
    .fill("1234");
  await page
    .getByRole("spinbutton", {
      name: "Parallel agent limit",
      exact: true,
    })
    .fill("3");
  await expect(page.getByTestId("model-config")).toContainText(
    '"goal_token_budget":1234',
  );
  await expect(page.getByTestId("model-config")).toContainText(
    '"goal_max_concurrent_agents":3',
  );
  await page.getByRole("button", { name: "Off", exact: true }).click();
  await expect(page.getByTestId("model-config")).toContainText(
    '"goal_max_concurrent_agents":0',
  );
  await page.getByRole("button", { name: "Auto", exact: true }).click();
  await expect(page.getByTestId("model-config")).toContainText(
    '"goal_max_concurrent_agents":null',
  );
  await switchLanguage(page, "ja");
  await expect(
    page.getByRole("spinbutton", { name: "トークン予算", exact: true }),
  ).toHaveValue("1234");

  expect(errors).toEqual([]);
});
test(`model selector JA→EN: safe load error follows locale and hides provider diagnostics`, async ({
  page,
}) => {
  await page.goto(`/?view=model-selector&lang=ja&model-error`);
  await page
    .getByRole("button", {
      name: "Raw Alpha 原文 · Raw reasoning 原文",
      exact: true,
    })
    .click({ timeout: 15000 });
  await expect(page.getByRole("alert")).toHaveText(
    "モデルの選択肢を読み込めませんでした。再試行する前に接続と選択したエージェントを確認してください。",
  );
  await switchLanguage(page, "en");
  await expect(page.getByRole("alert")).toHaveText(
    "Model options could not be loaded. Check the connection and selected agent before trying again.",
  );
  await expect(page.locator("body")).not.toContainText("Raw provider secret");
});
