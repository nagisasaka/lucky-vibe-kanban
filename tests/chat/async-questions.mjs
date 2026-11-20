// Offline UI fixture; the real delivery boundary is tested separately.
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";

const root = process.cwd();
const bundle = await build({
  entryPoints: ["tests/chat/async-questions.fixture.tsx"],
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
  chromiumSandbox: true,
});
try {
  const page = await browser.newPage();
  await page.route("**/*", (route) => route.abort());
  await page.setContent(html);
  await page.getByRole("button", { name: /未回答の質問/ }).click();
  await page.getByRole("button", { name: "Blue" }).click();
  await page.getByRole("button", { name: "回答を送信" }).click();
  await expect(page.locator("output")).toHaveText(
    JSON.stringify([{ answer: "Blue", retry: false }]),
  );
  await expect(page.locator('p[role="status"]')).toContainText(
    "二重に届く可能性",
  );
  await expect(page.locator("output")).toHaveText(
    JSON.stringify([{ answer: "Blue", retry: false }]),
  );
  await page.getByRole("button", { name: "同じ回答を再送" }).click();
  await expect(page.locator("output")).toHaveText(
    JSON.stringify([
      { answer: "Blue", retry: false },
      { answer: "Blue", retry: true },
    ]),
  );
  await expect(page.getByRole("button", { name: /未回答の質問/ })).toHaveCount(
    0,
  );
  console.log(
    "async question UI: explicit retry after unconfirmed delivery passed",
  );
} finally {
  await browser.close();
}
