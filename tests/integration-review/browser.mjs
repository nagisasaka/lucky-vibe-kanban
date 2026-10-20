// Invoked by isolated_review_browser: real app/router, SQLite and Git.
// Only conflict-provider events are supplied by the Rust fixture. No API mocks.
import { chromium, expect as baseExpect } from "@playwright/test";
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
const expect = baseExpect.configure({ timeout: 30_000 });
const base = process.env.LVK_REVIEW_BASE_URL;
assert.match(base, /^http:\/\/127\.0\.0\.1:\d+$/);
const id = process.env.LVK_REVIEW_RUN;
const mode = process.env.LVK_REVIEW_MODE;
const output = fileURLToPath(
  new URL("../../.build/integration-review/", import.meta.url),
);
await mkdir(output, { recursive: true });
async function api(path, method = "GET", body) {
  const response = await fetch(base + path, {
    method,
    redirect: "error",
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await response.json();
  assert.equal(response.status, 200, JSON.stringify(json));
  assert.equal(json.success, true, JSON.stringify(json));
  return json.data;
}
const config = (await api("/api/info")).config;
await api("/api/config", "PUT", {
  ...config,
  language: "JA",
  theme: mode === "cancel" ? "DARK" : "LIGHT",
  disclaimer_acknowledged: true,
  onboarding_acknowledged: true,
  remote_onboarding_acknowledged: true,
  telemetry_acknowledged: true,
  analytics_enabled: false,
  showcases: { seen_features: ["workspaces-guide"] },
});
const initial = await api(`/api/integrations/${id}`);
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
  ...(process.env.LVK_E2E_CHROMIUM
    ? { executablePath: process.env.LVK_E2E_CHROMIUM }
    : {}),
});
const shutdown = async () => {
  await browser.close();
  process.exit(143);
};
process.once("SIGTERM", shutdown);
process.once("SIGINT", shutdown);
const deadline = setTimeout(shutdown, 110_000);
const page = await browser.newPage({
  viewport:
    mode === "cancel"
      ? { width: 390, height: 844 }
      : { width: 1440, height: 1000 },
  locale: "ja-JP",
});
page.setDefaultTimeout(30000);
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
try {
  await page.goto(`${base}/workspaces/${initial.workspace_id}`);
  const explanation =
    mode === "empty"
      ? page.getByText("この解消の説明は保存されていません。", { exact: false })
      : page.getByText("feature の同じ行で A と C の追加が衝突しました。", {
          exact: false,
        });
  await expect(explanation.last()).toBeVisible();
  await expect(
    page
      .getByText(
        "未検証 — 明示的な指定により、Integrationの検証をすべて省略しています。",
      )
      .first(),
  ).toBeVisible();
  await expect(
    page.getByText("この候補の検証はまだ実行されていません。", {
      exact: false,
    }),
  ).toBeVisible();
  const details = page
    .locator("details")
    .filter({
      has: page.locator("summary", { hasText: "詳細：解消差分と候補" }),
    })
    .last();
  const summary = details.locator("summary");
  const diff = page.getByLabel("解消差分", { exact: true }).last();
  await expect(diff).not.toBeVisible();
  await page.screenshot({
    path: `${output}/${mode}-summary.png`,
    fullPage: true,
  });
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(details).toHaveAttribute("open", "");
  await expect(diff).toBeVisible();
  await expect(diff).toContainText("feature");
  await diff.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: `${output}/${mode}-details.png`,
    fullPage: true,
  });
  await summary.focus();
  await page.keyboard.press("Space");
  await expect(diff).not.toBeVisible();
  await page.reload();
  await expect(explanation.last()).toBeVisible();
  await expect(diff).not.toBeVisible();
  const approve = page.getByRole("button", {
    name: "承認して続行",
    exact: true,
  });
  if (mode === "stale") {
    // Retain the old page while a real replacement commit/review is saved.
    // Pause polling only at the browser request boundary, not by mocking responses.
    const held = [];
    await page.route(`**/api/integrations/${id}`, (route) => {
      held.push(route);
    });
    await writeFile(process.env.LVK_REVIEW_SIGNAL, "replace");
    await expect
      .poll(
        async () =>
          (await api(`/api/integrations/${id}`)).payload.result_commit,
      )
      .not.toBe(initial.payload.result_commit);
    const responsePromise = page.waitForResponse((r) =>
      r.url().endsWith(`/integrations/${id}/decision`),
    );
    await approve.click();
    const response = await responsePromise;
    assert.equal(response.status(), 400);
    assert.match(
      JSON.stringify(await response.json()),
      /SOURCE_CHANGED|Stale resolution review/,
    );
    assert.ok(
      !(await api(`/api/integrations/${id}`)).payload.reviews[0].decision,
    );
    for (const route of held) await route.continue();
    await page.unroute(`**/api/integrations/${id}`);
    await page.reload();
    await expect(
      page.getByText("これは古い候補です。この候補への承認はできません。"),
    ).toBeVisible();
    await expect(approve).toHaveCount(1); // Only the new candidate is actionable.
    await page.screenshot({
      path: `${output}/stale-rejected.png`,
      fullPage: true,
    });
  }
  if (mode === "approve") {
    const decision = page.waitForResponse((r) =>
      r.url().endsWith(`/integrations/${id}/decision`),
    );
    await approve.click();
    const approvedResponse = await decision;
    assert.equal(approvedResponse.status(), 200);
    await expect
      .poll(async () => (await api(`/api/integrations/${id}`)).status, {
        timeout: 60000,
      })
      .toBe("succeeded");
    const result = await api(`/api/integrations/${id}`);
    assert.equal(result.payload.published, true);
    assert.equal(result.payload.reviews[0].decision, "continue");
    const replay = await api(
      `/api/integrations/${id}/decision`,
      "POST",
      approvedResponse.request().postDataJSON(),
    );
    assert.equal(replay.status, "succeeded");
    assert.equal(replay.payload.result_commit, result.payload.result_commit);
  } else {
    await page
      .getByRole("button", { name: "Integrationを中止", exact: true })
      .click();
    await expect
      .poll(async () => (await api(`/api/integrations/${id}`)).status, {
        timeout: 60000,
      })
      .toBe("cancelled");
    assert.equal(
      (await api(`/api/integrations/${id}`)).payload.published,
      false,
    );
  }
  // Retained results remain visible after the execution controls return to the composer.
  await page.goto(`${base}/projects/${initial.project_id}`);
  await page.getByRole("button", { name: "統合 / 自動マージ" }).click();
  await expect(
    page.getByText(`${mode === "approve" ? "完了" : "中止済み"} · 未検証`, {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText(mode === "approve" ? /Git: 反映済み/ : /Git: 未反映/),
  ).toBeVisible();
  await page.screenshot({
    path: `${output}/${mode}-result.png`,
    fullPage: true,
  });
  assert.deepEqual(errors, [], "browser runtime errors");
  console.log(
    `PASS browser ${mode}: actual API/Git, Japanese explanation, keyboard disclosure, reload, decision/publication`,
  );
} catch (error) {
  await page
    .screenshot({ path: `${output}/${mode}-failure.png`, fullPage: true })
    .catch(() => {});
  await writeFile(
    `${output}/${mode}-failure.txt`,
    await page
      .locator("body")
      .innerText()
      .catch(() => ""),
  );
  throw error;
} finally {
  clearTimeout(deadline);
  await browser.close();
}
