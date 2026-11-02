import { type Page } from "@playwright/test";

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
