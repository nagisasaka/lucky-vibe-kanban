const language = "ja";
import { switchLanguage } from "./i18n-support";
import { test, expect } from "@playwright/test";
test(`async questions ${language}: retained draft and uncertain delivery follow locale`, async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto(`/?view=session-chat&mode=async&lang=${language}`);
  let ja = language === "ja";
  const pending = () =>
    page.getByRole("button", {
      name: ja ? "未回答の質問 2 件" : "2 unanswered questions",
      exact: true,
    });
  await expect(pending()).toBeVisible({ timeout: 15000 });
  await pending().click();
  const input = page.getByLabel("Raw question 原文", { exact: true });
  await page
    .getByRole("button", { name: "Raw choice 選択肢", exact: true })
    .click();
  await expect(input).toHaveValue("Raw choice 選択肢");
  await input.fill("  Raw answer 原文  ");
  await page
    .getByRole("button", {
      name: ja ? "回答を送信" : "Send answer",
      exact: true,
    })
    .first()
    .click();
  await expect(page.getByRole("alert")).toContainText(
    ja
      ? "回答の送信結果を確認できません。"
      : "Answer delivery could not be confirmed.",
  );
  await expect(input).toHaveValue("  Raw answer 原文  ");
  await expect(page.getByTestId("session-chat-events")).toHaveText(
    JSON.stringify([{ id: "raw-question-1", answer: "Raw answer 原文" }]),
  );
  await expect(
    page.getByText("Raw provider secret", { exact: false }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Raw blocked choice" }),
  ).toBeDisabled();
  await expect(
    page.getByLabel("Raw delivery question", { exact: true }),
  ).toBeDisabled();
  await switchLanguage(page, ja ? "en" : "ja");
  ja = !ja;
  await expect(pending()).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    ja
      ? "回答の送信結果を確認できません。"
      : "Answer delivery could not be confirmed.",
  );
  await expect(input).toHaveValue("  Raw answer 原文  ");
  await page.getByRole("button", { name: "async-disabled" }).click();
  const send = page
    .getByRole("button", {
      name: ja ? "回答を送信" : "Send answer",
      exact: true,
    })
    .first();
  await expect(send).toBeDisabled();
  await page.getByRole("button", { name: "async", exact: true }).click();
  await page.reload();
  ja = language === "ja";
  await expect(pending()).toBeVisible({ timeout: 15000 });
  await pending().click();
  await expect(input).toHaveValue("  Raw answer 原文  ");
  await page.getByRole("button", { name: "async-accept" }).click();
  await page
    .getByRole("button", {
      name: ja ? "回答を送信" : "Send answer",
      exact: true,
    })
    .first()
    .click();
  await expect(input).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: ja ? "未回答の質問 1 件" : "1 unanswered question",
      exact: true,
    }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: ja ? "未回答の質問 1 件" : "1 unanswered question",
      exact: true,
    }),
  ).toBeVisible({ timeout: 15000 });
  expect(errors).toEqual([]);
});
