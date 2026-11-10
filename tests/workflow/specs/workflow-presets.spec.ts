import { expect, test } from "@playwright/test";

for (const width of [1280, 390]) {
  test(`fixed presets keep stage parameters and frozen defaults at width ${width}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.route("**/workflow-preset-defaults", (route) =>
      route.fulfill({
        json: [
          "implement_review",
          "research_synthesize",
          "inventory_reduce",
        ].map((preset) => ({
          preset,
          additions: "Project evidence rule",
          revision: 3,
        })),
      }),
    );
    await page.goto("/?mode=workflow-presets");
    const method = page.getByRole("combobox").first();
    await expect(method).toHaveValue("single");
    await expect(
      method.locator('option[value="implement_review"]'),
    ).toBeEnabled();
    await method.selectOption("implement_review");
    await expect(
      page.getByRole("group", { name: "implement", exact: true }),
    ).toBeVisible();
    await page
      .getByLabel("review engine", { exact: true })
      .selectOption("CLAUDE_CODE");
    await page.getByLabel("review model", { exact: true }).fill("review-model");
    await page
      .getByLabel("review additions", { exact: true })
      .fill("Check API compatibility");
    const parameters = page.getByTestId("parameters");
    await expect(parameters).toContainText('"project_revision":3');
    await expect(parameters).toContainText("Project evidence rule");
    await expect(parameters).toContainText("Check API compatibility");
    await expect(parameters).toContainText('"engine":"CLAUDE_CODE"');
    await expect(
      page.getByRole("button", { name: "Start fixture" }),
    ).toBeEnabled();
    await method.selectOption("research_synthesize");
    // The perspective list is the only textarea outside a stage before launch additions.
    const perspectives = page
      .locator("label")
      .filter({ has: page.locator("textarea") })
      .first()
      .locator("textarea");
    await perspectives.fill("UX\nImplementation\nMaintenance");
    await expect(
      page.getByRole("group", { name: "research-3", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("group", { name: "synthesize", exact: true }),
    ).toBeVisible();
    await perspectives.fill("");
    await expect(
      page.getByRole("button", { name: "Start fixture" }),
    ).toBeDisabled();
    await method.selectOption("inventory_reduce");
    await expect(
      page.getByRole("group", { name: "inventory", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("group", { name: "reduce", exact: true }),
    ).toBeVisible();
    await expect(page.locator("fieldset")).toHaveCount(2);
    await expect(
      page.getByRole("button", { name: "Start fixture" }),
    ).toBeEnabled();
    await expect(
      page.getByRole("button", { name: /add node|connect|publish|merge/i }),
    ).toHaveCount(0);
  });
}
