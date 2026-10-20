import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { fileURLToPath } from "node:url";

// Component regression with canonical fixtures, not a provider/backend test.
test("board/chat run clocks stop, replay safely, and remain independent of Goal usage", async () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const result = await build({
    entryPoints: [root + "tests/chat/run-elapsed.fixture.tsx"],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: { "@": root + "packages/web-core/src", shared: root + "shared" },
    nodePaths: [root + "packages/web-core/node_modules"],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.clock.install({ time: new Date("2026-10-20T00:00:10Z") });
    await page.setContent('<div id="root"></div>');
    await page.addScriptTag({ content: result.outputFiles[0].text });
    await page.waitForFunction(() => !!window.setRuns);
    const run = (id, status, goal = false) => ({
      agent_run_id: id,
      session_id: "session",
      turn_id: id,
      created_at: "2026-10-20T00:00:00Z",
      updated_at: "2026-10-20T12:00:00Z",
      state: {
        status,
        last_run_attempt_id: id,
        projection_status: "current",
        goal: goal
          ? {
              status: "active",
              objective: "Goal",
              tokens_used: 0,
              time_used_seconds: 4,
              token_budget: null,
            }
          : null,
      },
    });
    const show = async (runs) => {
      await page.evaluate((runs) => window.setRuns(runs), runs);
      await expect(page.locator("section[id]")).toHaveCount(runs.length);
    };
    for (const goal of [false, true]) {
      let current = run("current", "running", goal);
      await show([current]);
      const elapsed = page.locator("#current .chat");
      await expect(elapsed).toContainText("s");
      const before = await elapsed.innerText();
      await page.clock.runFor(3000);
      assert.notEqual(await elapsed.innerText(), before);
      await expect(page.locator("#current .board")).toContainText(
        await elapsed.innerText(),
      );
      for (const status of [
        "awaiting_input",
        "awaiting_approval",
        "cancelling",
      ]) {
        current = { ...current, state: { ...current.state, status } };
        await show([current]);
        const prior = await elapsed.innerText();
        await page.clock.runFor(2000);
        assert.notEqual(await elapsed.innerText(), prior);
      }
      for (const status of [
        "succeeded",
        "failed",
        "cancelled",
        "crashed",
        "audit_failed",
      ]) {
        const stopped = {
          ...current,
          completed_at: "2026-10-20T00:00:12Z",
          state: { ...current.state, status },
        };
        await show([stopped, run("next", "running")]);
        await expect(elapsed).toHaveText("12s");
        await expect(page.locator("#current .board")).toContainText("12s");
        const nextBefore = await page.locator("#next .chat").innerText();
        await page.clock.runFor(3000);
        await expect(elapsed).toHaveText("12s");
        assert.notEqual(
          await page.locator("#next .chat").innerText(),
          nextBefore,
        );
        await page.evaluate(() => window.remount());
        await expect(elapsed).toHaveText("12s");
        await show([JSON.parse(JSON.stringify(stopped))]);
        await expect(elapsed).toHaveText("12s");
        if (goal)
          await expect(page.locator("#current")).toContainText(
            "Recorded time: 0m 4s",
          );
      }
    }
    await expect(page.locator("#stale")).toHaveText("12s");
    await expect(page.locator("#unknown")).toBeEmpty();
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
