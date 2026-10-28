import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
const root = process.cwd();
const preset = process.env.VITE_KANBAN_MOTION || "crisp";
const output = path.join(root, ".build/cross-column-" + preset);
await mkdir(output, { recursive: true });
const bundle = await build({
  entryPoints: ["tests/kanban/cross-column.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": root + "/packages/web-core/src" },
  nodePaths: [
    root + "/packages/web-core/node_modules",
    root + "/packages/ui/node_modules",
  ],
  loader: { ".css": "empty" },
  define: { "import.meta.env.VITE_KANBAN_MOTION": JSON.stringify(preset) },
});
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
const results = [];
let page;
try {
  for (const width of [360, 600]) {
    for (const action of [
      "drop",
      "cancel",
      "outside",
      "fixed",
      "touch",
      "keyboard-cancel",
    ]) {
      page = await browser.newPage({
        viewport: { width: 2000, height: 1000 },
        hasTouch: true,
      });
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.setContent('<div id="root"></div>');
      await page.evaluate((fixed) => {
        window.fixedSort = fixed;
      }, action === "fixed");
      await page.addStyleTag({
        content: `#root>div{display:grid;grid-template-columns:repeat(3,${width}px);gap:25px} .kanban-grid-column{container-type:inline-size} [data-kanban-column]{display:grid;grid-template-columns:${width >= 536 ? "1fr 1fr" : "1fr"};gap:8px;padding:8px;min-height:620px;align-content:start} [data-kanban-card]{border:1px solid #888;background:white;padding:8px} [data-kanban-ghost]{background:#ddd;width:260px}`,
      });
      await page.addScriptTag({ content: bundle.outputFiles[0].text });
      const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
      const ids = (col) =>
        page
          .locator(`[data-kanban-column="${col}"] [data-kanban-card]`)
          .evaluateAll((els) => els.map((e) => e.dataset.kanbanCard));
      await expect(card("a")).toBeVisible();
      const original = await card("d").boundingBox();
      const from = await card("a").boundingBox();
      await page.evaluate(() => {
        window.samples = [];
        window.record = true;
        const sample = () => {
          if (!window.record) return;
          window.samples.push(
            [
              ...document.querySelectorAll(
                '[data-kanban-column="B"] [data-kanban-card]',
              ),
            ].map((e) => ({
              id: e.dataset.kanbanCard,
              transform: getComputedStyle(e).transform,
              x: e.getBoundingClientRect().x,
              y: e.getBoundingClientRect().y,
            })),
          );
          requestAnimationFrame(sample);
        };
        sample();
      });
      let cdp;
      if (action === "touch") {
        cdp = await page.context().newCDPSession(page);
        const send = (type, point) =>
          cdp.send("Input.dispatchTouchEvent", {
            type,
            touchPoints: point ? [{ ...point, id: 1 }] : [],
          });
        const start = { x: from.x + 35, y: from.y + 30 };
        await send("touchStart", start);
        await page.waitForTimeout(300);
        for (let i = 1; i <= 20; i++) {
          await send("touchMove", {
            x: start.x + ((original.x + 30 - start.x) * i) / 20,
            y: start.y + ((original.y + 25 - start.y) * i) / 20,
          });
          await page.waitForTimeout(20);
        }
      } else if (action === "keyboard-cancel") {
        await card("a").focus();
        await page.keyboard.press("Space", { delay: 100 });
        for (let i = 0; i < (width >= 536 ? 2 : 1); i++)
          await page.keyboard.press("ArrowRight", { delay: 100 });
      } else {
        await page.mouse.move(from.x + 35, from.y + 30);
        await page.mouse.down();
        await page.mouse.move(from.x + 48, from.y + 30, { steps: 4 });
        await page.mouse.move(original.x + 30, original.y + 25, { steps: 20 });
      }
      if (action !== "fixed") {
        await expect
          .poll(() => ids("B"))
          .toEqual(
            action === "keyboard-cancel"
              ? ["a", "c", "d", "e"]
              : ["c", "a", "d", "e"],
          );
        await expect(card("a")).toHaveCount(1);
        await expect(page.locator("[data-kanban-ghost]")).toHaveCount(1);
        await expect
          .poll(async () => {
            const b = await card("d").boundingBox();
            return Math.abs(b.x - original.x) + Math.abs(b.y - original.y);
          })
          .toBeGreaterThan(20);
      } else {
        assert.deepEqual(await ids("B"), ["c", "d", "e"]);
      }
      assert.equal(
        await page.evaluate(() => window.drops.length),
        0,
        "hover must not commit",
      );
      await page.screenshot({
        path: path.join(output, `hover-${width}-${action}.png`),
      });
      if (action === "cancel" || action === "keyboard-cancel")
        await page.keyboard.press("Escape", { delay: 100 });
      if (action === "outside") await page.mouse.move(1980, 980, { steps: 10 });
      if (cdp) {
        await cdp.send("Input.dispatchTouchEvent", {
          type: "touchEnd",
          touchPoints: [],
        });
        await cdp.detach();
      } else await page.mouse.up();
      if (
        action === "cancel" ||
        action === "outside" ||
        action === "keyboard-cancel"
      ) {
        await expect.poll(() => ids("A")).toEqual(["a", "b"]);
        assert.deepEqual(await ids("B"), ["c", "d", "e"]);
        assert.equal(await page.evaluate(() => window.drops.length), 0);
        if (action === "keyboard-cancel") await expect(card("a")).toBeFocused();
      } else {
        await expect
          .poll(() => page.evaluate(() => window.drops.length))
          .toBe(1);
        await expect.poll(() => ids("B")).toEqual(["c", "a", "d", "e"]);
        assert.equal(
          (await page.evaluate(() => window.drops[0])).destination.index,
          1,
        );
        // A subsequent keyboard drag cancels without reload and retains focus.
        await card("a").focus();
        await page.keyboard.press("Space", { delay: 100 });
        await expect(card("a")).toHaveAttribute("aria-pressed", "true");
        await page.keyboard.press("Escape", { delay: 100 });
        await expect(page.locator("[data-kanban-ghost]")).toHaveCount(0);
        await expect(card("a")).toBeFocused();
        assert.equal(await page.evaluate(() => window.drops.length), 1);
      }
      await page.evaluate(() => {
        window.record = false;
      });
      const samples = await page.evaluate(() => window.samples);
      if (action !== "fixed")
        assert.ok(
          samples.some((frame) =>
            frame.some(
              (c) =>
                c.id === "d" &&
                c.transform !== "none" &&
                c.transform !== "matrix(1, 0, 0, 1, 0, 0)",
            ),
          ),
          "destination displacement must animate",
        );
      assert.deepEqual(errors, []);
      results.push({
        width,
        action,
        samples,
        drops: await page.evaluate(() => window.drops),
      });
      await page.close();
      page = null;
    }
  }
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(
    `PASS ${preset}: cross-column animated slot/drop/cancel/outside/fixed, one/two columns; no backend`,
  );
} catch (error) {
  await page?.screenshot({ path: path.join(output, "failure.png") });
  throw error;
} finally {
  await browser.close();
}
