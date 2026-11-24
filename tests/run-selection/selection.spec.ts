import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  // The fixture must never talk to an LVK instance or start an agent.
  await page.route("**/api/**", (route) => route.abort());
});

for (const scenario of ["configured", "profile"]) {
  test(`${scenario}: read-only inheritance, per-field override, reset and same value`, async ({
    page,
  }, testInfo) => {
    await page.goto(`/?scenario=${scenario}`);
    const picker = page.getByTestId("picker");
    const model = picker.getByRole("button").filter({ hasText: "Alpha" });
    await expect(model).toContainText("High");
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await page.getByText("送信値を確認").click();
    expect(
      JSON.parse(await page.getByTestId("sent").innerText()),
    ).not.toHaveProperty("model_id");
    expect(
      JSON.parse(await page.getByTestId("sent").innerText()),
    ).not.toHaveProperty("reasoning_id");
    expect(
      JSON.parse(await page.getByTestId("sent").innerText()),
    ).not.toHaveProperty("permission_policy");
    await model.click();
    await page.getByRole("button", { name: "High", exact: true }).click();
    await page.getByRole("menuitem", { name: "Medium", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(picker.locator(".text-brand")).toHaveText(["Medium"]);
    await model.click();
    await page.getByRole("button", { name: "Medium", exact: true }).click();
    await page.getByRole("menuitem", { name: "High", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    expect(
      JSON.parse(await page.getByTestId("draft").innerText()).reasoning_id,
    ).toBeNull();
    await model.click();
    await page.getByRole("button", { name: "Beta", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(picker).toContainText("未確認");
    await expect(picker.locator(".text-brand")).toHaveText(["Beta"]);
    await picker.getByRole("button").filter({ hasText: "Beta" }).click();
    await page.getByRole("button", { name: "Alpha", exact: true }).click();
    await page.keyboard.press("Escape");
    expect(
      JSON.parse(await page.getByTestId("draft").innerText()).model_id,
    ).toBeNull();
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await model.click();
    await page.getByRole("button", { name: "Beta", exact: true }).click();
    await page
      .getByRole("button", { name: "上書きを解除", exact: true })
      .click();
    await page.keyboard.press("Escape");
    await expect(picker).toContainText("Alpha");
    await expect(picker).toContainText("High");
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await picker.getByRole("button", { name: /権限:/ }).click();
    await page
      .getByRole("menuitem", {
        name: "信頼されていない操作を確認",
        exact: true,
      })
      .click();
    await expect(picker.locator(".text-brand")).toHaveText([
      "信頼されていない操作を確認",
    ]);
    await picker.getByRole("button", { name: /権限:/ }).click();
    await page
      .getByRole("menuitem", {
        name: scenario === "profile" ? "必要時に確認" : "確認しない",
        exact: true,
      })
      .click();
    expect(
      JSON.parse(await page.getByTestId("draft").innerText()).permission_policy,
    ).toBeNull();
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await picker.getByRole("button", { name: /権限:/ }).click();
    await page
      .getByRole("menuitem", {
        name: "信頼されていない操作を確認",
        exact: true,
      })
      .click();
    await picker.getByRole("button", { name: /権限:/ }).click();
    await page
      .getByRole("menuitem", { name: "上書きを解除", exact: true })
      .click();
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await page.screenshot({
      animations: "disabled",
      path: testInfo.outputPath("inherited-light.png"),
    });
  });
}

for (const scenario of ["empty", "resume", "claude"]) {
  test(`${scenario}: unknown defaults are not guessed from model candidates`, async ({
    page,
  }, testInfo) => {
    await page.goto(`/?scenario=${scenario}`);
    const picker = page.getByTestId("picker");
    await expect(picker).toContainText("未確認");
    await expect(picker.locator(".text-brand")).toHaveCount(0);
    await picker
      .getByRole("button")
      .filter({ hasText: "未確認" })
      .first()
      .click();
    await page.getByRole("button", { name: "Haiku", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(picker).toContainText("非対応");
    await expect(picker.locator(".text-brand")).toHaveText(["Haiku"]);
    expect(
      JSON.parse(await page.getByTestId("draft").innerText()).reasoning_id,
    ).toBeNull();
    if (scenario === "claude")
      await expect(picker).not.toContainText("必要時に確認");
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => document.documentElement.classList.add("dark"));
    await expect(picker).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      animations: "disabled",
      path: testInfo.outputPath("override-mobile-dark.png"),
    });
  });
}
