import assert from "node:assert/strict";
import { expect } from "@playwright/test";

// Real app routes, file APIs and conversation. Seeded disposable DB, no mocks.
export async function verifyMobileNavigation(page, base) {
  for (const width of [390, 320, 600]) {
    await page.setViewportSize({ width, height: 844 });
    const nav = page.locator("nav").filter({
      has: page.getByRole("button", { name: "Wksps", exact: true }),
    });
    const tab = (name) => nav.getByRole("button", { name, exact: true });
    for (const name of [
      "Wksps",
      "Chat",
      "Files",
      "Wiki",
      "TODO",
      "Diff",
      "Logs",
      "Git",
    ]) {
      await tab(name).click();
      await expect(tab(name)).toHaveAttribute("aria-current", "page");
      const surfaces = {
        Wksps: page
          .getByText("Workspaces", { exact: true })
          .filter({ visible: true }),
        Chat: page.getByRole("link", {
          name: "Open native source",
          exact: true,
        }),
        Files: page
          .getByRole("button", { name: "Close preview", exact: true })
          .or(page.getByPlaceholder("Search files"))
          .filter({ visible: true }),
        Wiki: page.getByText("No Wiki pages in this repository.", {
          exact: true,
        }),
        TODO: page.getByRole("heading", { name: "TODO list", exact: true }),
        Diff: page.getByText("No changes to display", { exact: true }),
        Logs: page.getByRole("region", { name: "Dev server", exact: true }),
        Git: page.getByText("Notes", { exact: true }).filter({ visible: true }),
      };
      await expect(surfaces[name].first()).toBeVisible();
      // Let workspace/session observations rerender the navbar.
      await page.waitForTimeout(350);
      await expect(tab(name)).toHaveAttribute("aria-current", "page");
      assert.ok(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        `no page overflow at ${width}: ${name}`,
      );
    }
    await page.screenshot({
      path: `.build/mobile-navigation/${width}-git.png`,
    });
    await tab("Files").click();
    const close = page.getByRole("button", {
      name: "Close preview",
      exact: true,
    });
    // A previous selection can remain visible on resize.
    if (await close.isVisible()) {
      await page
        .getByRole("button", { name: "Files", exact: true })
        .filter({ has: page.locator("svg") })
        .last()
        .click();
    }
    // File browser includes the real repository and directory nodes.
    const source = page
      .getByText("src", { exact: true })
      .filter({ visible: true });
    if (
      !(await page
        .getByText("example.ts", { exact: true })
        .filter({ visible: true })
        .count())
    )
      await source.click();
    await page
      .getByText("example.ts", { exact: true })
      .filter({ visible: true })
      .click();
    await expect(close).toBeVisible();
    await page.screenshot({
      path: `.build/mobile-navigation/${width}-files.png`,
    });
    await expect(
      page.getByText("const nativeSelected = 120;", { exact: true }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "Files", exact: true })
      .last()
      .click();
    await expect(close).not.toBeVisible();
    await tab("Chat").click();
    const link = page.getByRole("link", {
      name: "Open native source",
      exact: true,
    });
    for (let repeat = 0; repeat < 2; repeat++) {
      await link.click();
      await expect(tab("Files")).toHaveAttribute("aria-current", "page");
      await expect(close).toBeVisible();
      await expect(page.locator('[aria-current="location"]')).toBeInViewport();
      await page
        .getByRole("button", { name: "Files", exact: true })
        .last()
        .click();
      await expect(close).not.toBeVisible();
      await tab("Chat").click();
    }
    await tab("Chat").click();
    await expect(link).toBeVisible();
    // Reduced viewport exercises the composer layout; headless Chromium does
    // not provide a real OS keyboard.
    await page.setViewportSize({ width, height: 420 });
    const input = page
      .locator('[contenteditable="true"]')
      .filter({ visible: true });
    await input.focus();
    await expect(input).toBeInViewport();
    await expect(
      page.getByRole("button", { name: /^Send(?: unavailable:.*)?$/ }),
    ).toBeInViewport();
  }
  // Return to desktop and exercise the same conversation link.
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page
    .getByRole("link", { name: "Open native source", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Close preview", exact: true }),
  ).toBeVisible();
  await expect(page.locator('[aria-current="location"]')).toBeInViewport();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`${base}/workspaces/create`);
  const nav = page
    .locator("nav")
    .filter({ has: page.getByRole("button", { name: "Wksps", exact: true }) });
  for (const name of ["Files", "Wiki", "TODO", "Diff", "Logs", "Git"])
    await expect(nav.getByRole("button", { name, exact: true })).toHaveCount(0);
  await expect(
    nav.getByRole("button", { name: "Chat", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await nav.getByRole("button", { name: "Wksps", exact: true }).click();
  await expect(
    nav.getByRole("button", { name: "Wksps", exact: true }),
  ).toHaveAttribute("aria-current", "page");
  console.log(
    "Native mobile all tabs, tree/preview/back, repeated chat link, desktop return passed",
  );
}
