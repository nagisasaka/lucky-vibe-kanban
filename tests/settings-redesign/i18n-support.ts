import { expect, type Locator, type Page } from "@playwright/test";

export function switchLanguage(page: Page, language: string) {
  return page.evaluate(
    (locale) =>
      (
        window as unknown as {
          __setUiLanguage: (value: string) => Promise<unknown>;
        }
      ).__setUiLanguage(locale),
    language,
  );
}

// Keep the same polling and horizontal margins used by the mobile smoke tests.
export async function expectWithinWidth(
  target: Locator,
  width: number,
  margin = 0,
) {
  await expect
    .poll(async () => {
      const box = await target.boundingBox();
      return !!box && box.x >= margin && box.x + box.width <= width - margin;
    })
    .toBe(true);
}

// Preserve the existing whole-page overflow poll, independently of viewport size.
export async function expectNoHorizontalOverflow(page: Page) {
  await expect
    .poll(() =>
      page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    )
    .toBe(true);
}
