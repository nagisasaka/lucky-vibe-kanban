import { expect, test, type Page } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";

const directory = ".build/settings-visual-review";

async function settled(page: Page) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(
      document
        .getAnimations()
        .filter(
          (animation) =>
            animation.effect?.getComputedTiming().iterations !== Infinity,
        )
        .map((animation) => animation.finished.catch(() => undefined)),
    );
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
  });
  const dialog = page.getByRole("dialog", { name: "設定", exact: true });
  await expect(dialog).toHaveCSS("opacity", "1");
  await expect(dialog).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await expect(dialog).toHaveCSS("backdrop-filter", "none");
}

async function open(page: Page, query: string) {
  await page.goto("/" + query);
  await page.getByRole("button", { name: "設定を開く", exact: true }).click();
  return page.getByRole("dialog", { name: "設定", exact: true });
}

async function capture(page: Page, name: string) {
  await settled(page);
  const dialog = page.getByRole("dialog", { name: "設定", exact: true });
  await expect(
    dialog.getByText("• 未保存の変更があります", { exact: true }),
  ).toHaveCount(0);
  await expect(
    dialog.getByText("✓ 設定が正常に保存されました！", { exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "保存", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const surface = await dialog.evaluate((element) => ({
    opacity: getComputedStyle(element).opacity,
    background: getComputedStyle(element).backgroundColor,
    backdrop: getComputedStyle(element).backdropFilter,
    runningAnimations: document
      .getAnimations()
      .filter((animation) => animation.playState === "running").length,
    viewport: { width: innerWidth, height: innerHeight },
    capture: { width: element.clientWidth, height: element.clientHeight },
  }));
  await mkdir(directory, { recursive: true });
  await writeFile(
    `${directory}/${name}.json`,
    JSON.stringify(surface, null, 2),
  );
  await dialog.screenshot({
    path: `${directory}/${name}.png`,
    animations: "disabled",
  });
}

test("PC: all categories and notification form are settled and opaque", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 1040 });
  const dialog = await open(page, "?section=notifications");
  await expect(page.locator("#sound-enabled")).not.toBeChecked();
  await expect(
    dialog
      .getByRole("navigation")
      .getByRole("button", { name: "共有リソース・復旧", exact: true }),
  ).toBeInViewport();
  await expect(
    dialog
      .getByRole("navigation")
      .getByRole("button", { name: "認証・メンバー", exact: true }),
  ).toBeInViewport();
  await capture(page, "pc-categories-notifications");
});

test("PC: authentication form does not show background text after settling", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 1040 });
  await open(page, "?section=access&auth=basic");
  await expect(
    page.getByLabel("新しいパスワード（12文字以上）", { exact: true }),
  ).toHaveValue("");
  await expect(page.getByLabel("ユーザー名", { exact: true })).toHaveValue(
    "demo",
  );
  await capture(page, "pc-authentication");
});

test("mobile: category list is settled and scrolls to the final group", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const dialog = await open(page, "");
  const nav = dialog.getByRole("navigation");
  await expect(nav).toBeVisible();
  const last = nav.getByRole("button", {
    name: "共有リソース・復旧",
    exact: true,
  });
  await last.scrollIntoViewIfNeeded();
  await expect(last).toBeInViewport();
  await nav.evaluate((element) => element.scrollTo(0, 0));
  await capture(page, "mobile-categories");
});

test("mobile: clean execution permissions form is readable with no dirty bar", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "?section=mcp-integration");
  await expect(page.locator("#runtime-files")).toBeChecked();
  await expect(page.locator("#runtime-execution")).not.toBeChecked();
  await capture(page, "mobile-permissions");
});

test("saving then editing or discarding clears stale success feedback", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 1040 });
  await open(page, "?section=notifications");
  await page.locator("#sound-enabled").check();
  await page.getByRole("button", { name: "保存", exact: true }).click();
  const success = page.getByText("✓ 設定が正常に保存されました！", {
    exact: true,
  });
  await expect(success).toBeVisible();
  await page.locator("#sound-enabled").uncheck();
  await expect(page.getByRole("status")).toContainText("未保存");
  await expect(success).toHaveCount(0);
  await page.getByRole("button", { name: "破棄", exact: true }).click();
  await expect(page.locator("#sound-enabled")).toBeChecked();
  await expect(
    page.getByText("• 未保存の変更があります", { exact: true }),
  ).toHaveCount(0);
  await expect(success).toHaveCount(0);
});

test("dark settings surface is opaque and respects the theme token", async ({
  page,
}) => {
  await open(page, "?section=access&auth=basic&theme=dark");
  await page.evaluate(async () => {
    await Promise.all(
      document
        .getAnimations()
        .filter(
          (animation) =>
            animation.effect?.getComputedTiming().iterations !== Infinity,
        )
        .map((animation) => animation.finished.catch(() => undefined)),
    );
  });
  const dialog = page.getByRole("dialog", { name: "設定", exact: true });
  expect(
    await dialog.evaluate(
      (element) => getComputedStyle(element).backgroundColor,
    ),
  ).toMatch(/^rgb\(/);
  await expect(dialog).toHaveCSS("opacity", "1");
  await expect(dialog).toHaveCSS("backdrop-filter", "none");
});
