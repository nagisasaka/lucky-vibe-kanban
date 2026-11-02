import { expectWithinWidth } from "./i18n-support";
import { test, expect } from "@playwright/test";
test(`webview menu JA→EN: live labels preserve clipboard bytes and editability gates`, async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => {
    const writes: string[] = [];
    const commands: string[] = [];
    Object.defineProperty(window, "__clipboardWrites", { value: writes });
    Object.defineProperty(window, "__editingCommands", { value: commands });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          writes.push(text);
        },
        readText: async () => "Clipboard 原文",
      },
    });
    document.execCommand = (command: string) => {
      commands.push(command);
      return true;
    };
  });
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto(`/?view=context-menu-frame&lang=ja`);
  const frame = page.frameLocator('iframe[title="Editor fixture"]');
  const editable = frame.getByTestId("editable");
  await expect(editable).toHaveValue("Raw 原文 abc", { timeout: 15000 });
  await editable.focus();
  await editable.evaluate((element: HTMLTextAreaElement) =>
    element.setSelectionRange(0, 3),
  );
  await editable.click({ button: "right", position: { x: 3, y: 8 } });
  await expect(
    frame.getByRole("button", {
      name: "切り取り",
      exact: true,
    }),
  ).toBeVisible();
  await editable.evaluate(async (_element, language) => {
    await (
      window as unknown as {
        __setUiLanguage: (lang: string) => Promise<unknown>;
      }
    ).__setUiLanguage(language);
  }, "en");
  await frame.getByRole("button", { name: "Copy", exact: true }).click();
  await expect
    .poll(() =>
      editable.evaluate(
        () =>
          (window as unknown as { __clipboardWrites: string[] })
            .__clipboardWrites,
      ),
    )
    .toEqual(["Raw"]);
  await expect(editable).toHaveValue("Raw 原文 abc");
  await editable.focus();
  await editable.evaluate((element: HTMLTextAreaElement) =>
    element.setSelectionRange(0, 3),
  );
  await editable.click({ button: "right", position: { x: 3, y: 8 } });
  await frame.getByRole("button", { name: "Cut", exact: true }).click();
  await expect(editable).toHaveValue(" 原文 abc");
  await editable.focus();
  await editable.evaluate((element: HTMLTextAreaElement) =>
    element.setSelectionRange(0, 0),
  );
  // Open at the captured caret without a pointer click relocating it.
  await editable.dispatchEvent("contextmenu", {
    button: 2,
    clientX: 20,
    clientY: 20,
  });
  await expect(
    frame.getByRole("button", { name: "Cut", exact: true }),
  ).toHaveCount(0);
  await frame.getByRole("button", { name: "Paste", exact: true }).click();
  await expect(editable).toHaveValue("Clipboard 原文 原文 abc");
  for (const [command, label] of [
    ["undo", "Undo"],
    ["redo", "Redo"],
    ["selectAll", "Select All"],
  ]) {
    await editable.click({ button: "right", position: { x: 3, y: 8 } });
    await frame.getByRole("button", { name: label, exact: true }).click();
    await expect
      .poll(() =>
        editable.evaluate(
          () =>
            (window as unknown as { __editingCommands: string[] })
              .__editingCommands,
        ),
      )
      .toContain(command);
  }
  await frame.getByTestId("readonly").click({ button: "right" });
  await expect(
    frame.getByRole("button", { name: "Cut", exact: true }),
  ).toHaveCount(0);
  await expect(
    frame.getByRole("button", {
      name: "Paste",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(frame.getByTestId("raw-text")).toHaveText(
    "Raw model output 原文",
  );
  await expectWithinWidth(
    frame.getByRole("button", {
      name: "Select All",
      exact: true,
    }),
    360,
  );
  // Top-level pages retain the browser's native context menu.
  await page.goto(`/?view=context-menu&lang=ja`);
  await page.getByTestId("editable").click({ button: "right" });
  await expect(page.getByRole("button")).toHaveCount(0);
  expect(errors).toEqual([]);
});
