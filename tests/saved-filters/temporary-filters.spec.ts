import { test, expect } from "@playwright/test";

test("nested temporary filter menus release pointer blocking after Escape", async ({
  page,
}) => {
  test.setTimeout(90000); // The real dialog imports app modules on a cold Vite cache.
  page.on("pageerror", (error) => console.error(error.message));
  await page.goto("/?temporary-filters");
  for (const label of [/^優先度/, /^タグ$/, /^並べ替え:/]) {
    await page.getByRole("button", { name: "Open temporary filters" }).click();
    const dialog = page.getByRole("dialog", { name: "絞り込み", exact: true });
    await dialog.getByRole("button", { name: label }).click();
    await expect(page.getByRole("menu")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("menu", { includeHidden: true })).toHaveCount(
      0,
    );
    if (await dialog.isVisible())
      await dialog.getByRole("button", { name: "閉じる", exact: true }).click();
    await expect(page.getByRole("dialog", { includeHidden: true })).toHaveCount(
      0,
    );
    await expect(page.locator("body")).not.toHaveCSS("pointer-events", "none");
    // A real mouse click must work after the modal closes (no force/DOM events).
    await page.getByRole("button", { name: "Open temporary filters" }).click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "閉じる", exact: true }).click();
    await expect(dialog).toHaveCount(0);
  }
});
