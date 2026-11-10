// Offline production-component regression. No app DB, provider or live server.
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";

const root = process.cwd();
const bundle = await build({
  entryPoints: ["tests/chat/native-questions.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  nodePaths: [`${root}/packages/web-core/node_modules`],
  alias: { "@": `${root}/packages/web-core/src`, shared: `${root}/shared` },
});
const html = `<meta charset="utf-8"><div id="root"></div><script>${bundle.outputFiles[0].text.replaceAll("</script", "<\\/script")}</script>`;
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route("**/*", (route) => route.abort());
  await page.setContent(html);
  await expect(page.getByText("一つ目", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Blue" })).toHaveAttribute(
    "title",
    "最初の選択肢",
  );
  await page.getByRole("button", { name: "Blue" }).click();
  await expect(page.getByText("二つ目", { exact: true })).toBeVisible();
  await expect(page.locator("output")).toHaveText("pending");
  await page.getByRole("button", { name: "Green" }).click();
  await expect(page.locator("output")).toHaveText(
    JSON.stringify({
      answers: [
        { question: "__proto__", answer: ["Blue"] },
        { question: "constructor", answer: ["Green"] },
      ],
    }),
  );
  await page.setContent(html);
  await page.getByRole("button", { name: "Expire" }).click();
  await expect(page.getByRole("button", { name: "Blue" })).toBeDisabled();
  await expect(page.locator("output")).toHaveText("pending");
  expect(errors).toEqual([]);
  console.log(
    "native question UI: identity, options, multi-answer and expiry passed",
  );
} finally {
  await browser.close();
}
