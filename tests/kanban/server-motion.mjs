import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
const output = ".build/server-motion-component";
await mkdir(output, { recursive: true });
const { outputFiles } = await build({
  entryPoints: ["tests/kanban/server-motion.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": process.cwd() + "/packages/web-core/src" },
  nodePaths: [process.cwd() + "/packages/web-core/node_modules"],
  loader: { ".css": "empty" },
  define: { "import.meta.env.VITE_KANBAN_MOTION": '"crisp"' },
});
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
const evidence = [];
let page;
try {
  for (const width of [360, 535, 536, 600]) {
    page = await browser.newPage({
      viewport: { width: 2000, height: 900 },
      hasTouch: true,
    });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.setContent('<div id="root"></div>');
    await page.evaluate((w) => (window.columnWidth = w), width);
    await page.addStyleTag({
      content: `#root{width:1900px}#root #scroll>div>div{display:flex;gap:8px}section{flex-shrink:0}.kanban-grid-column{container-type:inline-size}.kanban-grid-cards{display:grid;grid-template-columns:1fr;gap:8px;padding:8px;align-content:start;min-height:500px}@container(min-width:536px){.kanban-grid-cards{grid-template-columns:1fr 1fr}}[data-kanban-card]{border:1px solid #aaa;background:white;padding:8px;box-sizing:border-box}[data-kanban-motion]{min-width:0;align-self:start}`,
    });
    await page.evaluate(() => (window.cached = true));
    await page.addScriptTag({ content: outputFiles[0].text });
    const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
    const node = (id) => page.locator(`[data-kanban-motion="${id}"]`);
    const quiet = async () => {
      await expect(page.locator("[data-kanban-animating]")).toHaveCount(0);
      await expect(page.locator("[data-kanban-server-ghost]")).toHaveCount(0);
    };
    const change = async (next, scope) => {
      await page.evaluate(({ next, scope }) => window.change(next, scope), {
        next,
        scope,
      });
    };
    const initial = { A: ["a", "b", "c"], B: ["d", "e"], C: [] };
    const reordered = { A: ["c", "a", "b"], B: ["d", "e"], C: [] };
    await expect(card("a")).toBeVisible();
    await change(reordered);
    await quiet(); // first fresh response
    await change(initial);
    await expect(node("a")).toHaveAttribute("data-kanban-animating", "true");
    await quiet();
    // Continuous updates start from the visible position, not the last destination.
    await change(reordered);
    await page.waitForTimeout(90);
    const before = await node("a").boundingBox();
    await change(initial);
    const after = await node("a").boundingBox();
    assert.ok(
      Math.abs(before.y - after.y) < 40,
      "retarget starts near current frame",
    );
    await quiet();
    // Both source and destination siblings move in a single commit, and focus
    // remains on the real corresponding nested control across parent changes.
    await card("b").getByRole("button").focus();
    const cross = { A: ["a", "c"], B: ["b", "d", "e"], C: [] };
    await change(cross);
    await expect(page.locator('[data-kanban-server-ghost="b"]')).toHaveCount(1);
    await expect(card("b").getByRole("button")).toBeFocused();
    await expect(node("c")).toHaveAttribute("data-kanban-animating", "true");
    await expect(node("d")).toHaveAttribute("data-kanban-animating", "true");
    await page.screenshot({ path: `${output}/cross-${width}-mid.png` });
    await quiet();
    await change(initial);
    await quiet();
    // An in-flight crossing that is reordered inside its destination must
    // continue through the same portal, without jumping to the hidden real node.
    await change(cross);
    await expect(page.locator('[data-kanban-server-ghost="b"]')).toHaveCount(1);
    await page.waitForTimeout(90);
    await change({ A: ["a", "c"], B: ["d", "b", "e"], C: [] });
    await expect(page.locator('[data-kanban-server-ghost="b"]')).toHaveCount(1);
    await quiet();
    await change(initial);
    await quiet();
    // Multiple simultaneous reparentings share one commit and no history queue.
    await change({ A: ["a"], B: ["b", "c", "d", "e"], C: [] });
    await expect(page.locator("[data-kanban-server-ghost]")).toHaveCount(2);
    await quiet();
    await change(initial);
    await quiet();
    // A scope switch, failure and recovery all establish quiet baselines.
    await change(reordered, "title");
    await quiet();
    await page.evaluate(() => window.fail());
    await change(initial);
    await quiet();
    await change(reordered);
    await expect(node("a")).toHaveAttribute("data-kanban-animating", "true");
    await quiet();
    // visibility events model the browser's hidden/visible boundary; request-start
    // timestamps are independently covered by the gate unit test.
    await page.evaluate(() =>
      document.dispatchEvent(new Event("visibilitychange")),
    );
    await change(initial);
    await quiet();
    await change(reordered);
    await quiet();
    // Reduced motion toggled while running cancels immediately.
    await change(initial);
    await expect(node("a")).toHaveAttribute("data-kanban-animating", "true");
    await page.emulateMedia({ reducedMotion: "reduce" });
    await quiet();
    await change(reordered);
    await quiet();
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.evaluate(
      () =>
        new Promise((r) =>
          requestAnimationFrame(() => requestAnimationFrame(r)),
        ),
    );
    // Synchronous stop at drag start clears automatic transforms before the
    // production Grid captures card rectangles. Existing keyboard focus retained.
    await change(initial);
    await expect(node("a")).toHaveAttribute("data-kanban-animating", "true");
    await card("a").focus();
    await page.keyboard.press("Space");
    await expect(card("a")).toHaveAttribute("aria-pressed", "true");
    await quiet();
    assert.equal(
      await node("a").evaluate((n) => getComputedStyle(n).transform),
      "none",
    );
    await page.keyboard.press("Escape");
    await expect(card("a")).toBeFocused();
    await quiet();
    // Scroll/width changes cancel a cross-column ghost and restore the real card.
    await change(cross);
    await expect(page.locator("[data-kanban-server-ghost]")).toHaveCount(1);
    await page.evaluate(() => {
      document.getElementById("root").style.width = "900px";
      document.getElementById("scroll").scrollLeft = 30;
    });
    await quiet();
    await page.evaluate(() => {
      document.getElementById("root").style.width = "1900px";
      document.getElementById("scroll").scrollLeft = 0;
    });
    await page.evaluate(
      () =>
        new Promise((r) =>
          requestAnimationFrame(() => requestAnimationFrame(r)),
        ),
    );
    await change(initial);
    await expect(page.locator("[data-kanban-server-ghost]")).toHaveCount(1);
    await page.evaluate(
      (w) => (document.querySelector("section").style.width = w + 1 + "px"),
      width,
    );
    await quiet();
    assert.equal(await node("b").evaluate((n) => n.style.opacity), "");
    // Offscreen destinations never produce a flight across the viewport.
    await page.evaluate(
      () =>
        (document.querySelectorAll("section")[2].style.marginLeft = "2500px"),
    );
    await change({ A: ["a", "c"], B: ["d", "e"], C: ["b"] });
    await quiet();
    assert.equal(await card("b").count(), 1);
    assert.deepEqual(errors, []);
    evidence.push({
      width,
      result: "passed",
      cases: [
        "fresh/scope/recovery",
        "retarget",
        "cross-column siblings",
        "nested focus",
        "reduced motion",
        "drag-start cancel",
        "keyboard cancel",
        "scroll/resize cleanup",
        "offscreen",
      ],
    });
    await page.close();
  }
  await writeFile(`${output}/results.json`, JSON.stringify(evidence, null, 2));
  console.log(evidence);
} catch (e) {
  await page?.screenshot({ path: `${output}/failure.png` }).catch(() => {});
  throw e;
} finally {
  await browser.close();
}
