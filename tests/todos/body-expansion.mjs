import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import path from "node:path";

export async function checkBodyExpansion({
  page,
  browser,
  webBase,
  project,
  grid,
  api,
  todoPath,
  output,
}) {
  const texts = [
    "短いメモ",
    "一行\n二行\n三行\n四行",
    "一行\n二行\n三行\n四行\n五行\n六行",
    "日本語の長文を折り返して表示します。".repeat(60),
    "x".repeat(1200),
  ];
  const fixtures = [];
  for (const body of texts)
    fixtures.push(
      (
        await api(todoPath, "POST", {
          id: randomUUID(),
          category: "00-body",
          body,
        })
      ).data,
    );
  const row = (i) => grid.locator(`[data-todo-id="${fixtures[i].id}"]`);
  const bodyButton = (r) =>
    r.locator("button").filter({ has: page.locator("span[id]") });
  await expect(row(4)).toBeVisible({ timeout: 15000 });
  for (const width of [1600, 390, 320]) {
    await page.setViewportSize({ width, height: 1000 });
    for (let i = 0; i < fixtures.length; i++) {
      const r = row(i);
      const more = r.getByRole("button", { name: "もっと見る", exact: true });
      if (i < 2) {
        await expect(more).toHaveCount(0);
      } else {
        await expect(more).toHaveAttribute("aria-expanded", "false");
        const collapsed = await bodyButton(r).boundingBox();
        assert.ok(
          await bodyButton(r).evaluate((el) => {
            const span = el.querySelector("span");
            return (
              span.clientHeight <=
                parseFloat(getComputedStyle(span).lineHeight) * 4 + 1 &&
              span.scrollHeight > span.clientHeight &&
              span.scrollWidth <= span.clientWidth
            );
          }),
          "four rendered lines, actual truncation, no horizontal overflow",
        );
        await more.focus();
        await page.keyboard.press("Enter");
        const less = r.getByRole("button", { name: "折りたたむ", exact: true });
        await expect(less).toHaveAttribute("aria-expanded", "true");
        assert.ok(
          (await bodyButton(r).boundingBox()).height > collapsed.height,
        );
        await expect(r.locator("textarea")).toHaveCount(0);
        await expect(r.getByRole("checkbox")).not.toBeChecked();
        await less.press("Space");
        await expect(more).toHaveAttribute("aria-expanded", "false");
      }
    }
    assert.ok(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    );
  }
  // One click enters the full original text in every display state.
  await page.setViewportSize({ width: 1600, height: 1000 });
  for (const [i, expand] of [
    [0, false],
    [2, false],
    [3, true],
    [4, false],
  ]) {
    const r = row(i);
    if (expand)
      await r.getByRole("button", { name: "もっと見る", exact: true }).click();
    await bodyButton(r).click();
    await expect(r.getByLabel("TODOを編集", { exact: true })).toHaveValue(
      texts[i],
    );
    await r.getByLabel("TODOを編集", { exact: true }).fill("未保存");
    await r.getByRole("button", { name: "キャンセル", exact: true }).click();
    assert.equal(
      (await api(`${todoPath}/${fixtures[i].id}`)).data.body,
      texts[i],
    );
    await bodyButton(r).click();
    const saved = texts[i] + "\n保存した追記";
    await r.getByLabel("TODOを編集", { exact: true }).fill(saved);
    await r.getByRole("button", { name: "保存", exact: true }).click();
    await expect(r.locator("textarea")).toHaveCount(0);
    await expect
      .poll(async () => (await api(`${todoPath}/${fixtures[i].id}`)).data.body)
      .toBe(saved);
  }
  await grid.getByLabel("完了済みも表示", { exact: true }).check();
  const r = row(2);
  await r.getByRole("button", { name: "もっと見る", exact: true }).click();
  await r.getByRole("checkbox").click();
  await expect(r.getByRole("checkbox")).toBeChecked();
  await expect(r.locator("textarea")).toHaveCount(0);
  await r.getByRole("button", { name: "TODOの操作", exact: true }).click();
  await expect(
    page.getByRole("menuitem", { name: "編集", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(r.locator("textarea")).toHaveCount(0);
  // A selection must survive the click generated at the end of a drag.
  await bodyButton(r).evaluate((el) => {
    const range = document.createRange();
    range.selectNodeContents(el);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    el.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 }));
  });
  await expect(r.locator("textarea")).toHaveCount(0);
  await page.evaluate(() => getSelection().removeAllRanges());
  await page.screenshot({
    path: path.join(output, "screenshots/body-expanded.png"),
    fullPage: true,
  });
  // Reflow while expanded must still measure the collapsed four-line version.
  const resize = row(1);
  await bodyButton(resize).evaluate(
    (el) => (el.parentElement.style.width = "12px"),
  );
  await expect(
    resize.getByRole("button", { name: "もっと見る", exact: true }),
  ).toBeVisible();
  await resize.getByRole("button", { name: "もっと見る", exact: true }).click();
  await bodyButton(resize).evaluate(
    (el) => (el.parentElement.style.width = ""),
  );
  await expect(
    resize.getByRole("button", { name: "折りたたむ", exact: true }),
  ).toHaveCount(0);
  await bodyButton(resize).evaluate(
    (el) => (el.parentElement.style.width = "12px"),
  );
  await expect(
    resize.getByRole("button", { name: "折りたたむ", exact: true }),
  ).toBeVisible();
  await bodyButton(resize).evaluate(
    (el) => (el.parentElement.style.width = ""),
  );
  // A late font load can change wrapping without changing the clamped height.
  const fontFixture = (
    await api(todoPath, "POST", {
      id: randomUUID(),
      category: "00-body",
      body: "i".repeat(120),
    })
  ).data;
  fixtures.push(fontFixture);
  const fontRow = grid.locator(`[data-todo-id="${fontFixture.id}"]`);
  await expect(fontRow).toBeVisible({ timeout: 15000 });
  await bodyButton(fontRow).evaluate((el) => {
    Object.assign(el.parentElement.style, {
      width: "120px",
      fontFamily: "sans-serif",
      fontSize: "16px",
      lineHeight: "24px",
    });
  });
  await expect(
    fontRow.getByRole("button", { name: "もっと見る", exact: true }),
  ).toHaveCount(0);
  await bodyButton(fontRow).evaluate(async (el) => {
    const font = new FontFace("TodoRegressionMono", 'local("Liberation Mono")');
    document.fonts.add(font);
    el.parentElement.style.fontFamily = "TodoRegressionMono, sans-serif";
    await document.fonts.load("16px TodoRegressionMono");
    await document.fonts.ready;
  });
  await expect(
    fontRow.getByRole("button", { name: "もっと見る", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: path.join(output, "screenshots/body-collapsed.png"),
    fullPage: true,
  });
  const touch = await browser.newContext({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
  });
  try {
    const mobile = await touch.newPage();
    await mobile.goto(`${webBase}/projects/${project}`);
    await mobile.getByRole("button", { name: "TODOリスト", exact: true }).tap();
    const mobileRow = mobile.locator(`[data-todo-id="${fixtures[4].id}"]`);
    await mobileRow
      .getByRole("button", { name: "もっと見る", exact: true })
      .tap();
    await expect(
      mobileRow.getByRole("button", { name: "折りたたむ", exact: true }),
    ).toBeVisible();
    await expect(mobileRow.locator("textarea")).toHaveCount(0);
    await mobileRow
      .locator("button")
      .filter({ has: mobile.locator("span[id]") })
      .tap();
    await expect(mobileRow.locator("textarea")).toHaveValue(
      texts[4] + "\n保存した追記",
    );
  } finally {
    await touch.close();
  }
  for (const f of fixtures) {
    const current = (await api(`${todoPath}/${f.id}`)).data;
    await api(
      `${todoPath}/${f.id}?expected_revision=${current.revision}`,
      "DELETE",
    );
  }
  await expect(row(0)).toHaveCount(0, { timeout: 15000 });
  console.log(
    "PASS body: four lines, short/Japanese/newlines/unbroken, keyboard/touch, resize, full edit/save/cancel, selection, completion/menu",
  );
}
