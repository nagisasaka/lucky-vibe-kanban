import { switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";

test("Session chat JA→EN: draft and callback bytes survive locale and blocked send", async ({
  page,
}) => {
  await page.goto("/?view=session-chat&lang=ja");
  const editor = page.getByTestId("chat-editor");
  const box = page.getByTestId("session-chat");
  await expect(editor).toHaveAttribute(
    "placeholder",
    "このタスクへの追加の指示を入力…",
    { timeout: 15000 },
  );
  await expect(box).toContainText("変更ファイル1,234件");
  await expect(editor).toHaveValue("Raw draft 原文");
  await switchLanguage(page, "en");
  await expect(editor).toHaveAttribute(
    "placeholder",
    "Continue working on this task...",
  );
  await expect(editor).toHaveValue("Raw draft 原文");
  await page.getByRole("button", { name: "running", exact: true }).click();
  await box.getByRole("button", { name: "Steer", exact: true }).click();
  const events = page.getByTestId("session-chat-events");
  const sent = '[{"action":"steer","value":"Raw draft 原文"}]';
  await expect(events).toHaveText(sent);
  await page.getByRole("button", { name: "blocked", exact: true }).click();
  const reason = "The agent is still running. Queue a follow-up instead.";
  await expect(
    box.getByRole("button", {
      name: "Send unavailable: " + reason,
      exact: true,
    }),
  ).toBeDisabled();
  await editor.press("Control+Enter");
  await expect(events).toHaveText(sent);
});
