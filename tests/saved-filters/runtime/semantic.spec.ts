import { test, expect, type Request } from "@playwright/test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { createHash } from "node:crypto";

const handoffPath = process.env.LOCAL460_HANDOFF;
if (!handoffPath) throw new Error("Explicit isolated handoff required");
const h = JSON.parse(readFileSync(handoffPath, "utf8"));
const f = h.semantic_filter;
if (
  h.provider_allowed !== true ||
  !f?.natural_language ||
  process.env.LOCAL460_SEMANTIC_OPT_IN !== f.id ||
  process.env.LOCAL460_EXPECTED_SOURCE !== h.source_sha ||
  !/^[a-f0-9]{40}$/.test(h.source_sha) ||
  !/^http:\/\/127\.0\.0\.1:\d+$/.test(h.backend_url) ||
  new URL(h.backend_url).port === "4020" ||
  h.frontend_url !== h.backend_url ||
  h.baseline?.agent_runs !== 0 ||
  h.baseline?.semantic_cache_rows_after_get !== 0 ||
  h.baseline?.initial_results?.state !== "unevaluated"
)
  throw new Error("Provider opt-in, exact source and clean baseline required");
const base = h.backend_url;
const project = h.projects.semantic.id;
const path = `/api/projects/${project}/saved-filters/${f.id}`;
const out = resolve(
  process.env.LOCAL460_OUTPUT ?? ".build/saved-filters/semantic",
);
mkdirSync(out, { recursive: true });
// Public handoff contains only allowlisted observations, never auth/headers.
const publicPath = resolve(
  dirname(handoffPath),
  "ui-semantic-observation.json",
);
const fingerprint = (value: unknown) =>
  createHash("sha256").update(JSON.stringify(value)).digest("hex");

test("opt-in real provider: concurrent UI/HTTP launch and unchanged cache reuse", async ({
  page,
  request,
}) => {
  const posts: any[] = [];
  const evidence: any = {
    source_sha: h.source_sha,
    served_assets_sha256: h.served_assets_sha256,
    project_id: project,
    filter_id: f.id,
    definition_revision: f.revision,
    context_sha256: fingerprint(f.context),
    definition_sha256: fingerprint(f),
    baseline: h.baseline,
    posts,
    screenshots: [],
    core_run_cache_comparison: "pending core read-only comparison",
  };
  const save = () => {
    const data = JSON.stringify(evidence, null, 2) + "\n";
    writeFileSync(`${out}/semantic-observation.json`, data);
    writeFileSync(publicPath, data);
  };
  const get = async (suffix = "") => {
    const response = await request.get(base + path + suffix);
    expect(response.ok()).toBeTruthy();
    return (await response.json()).data;
  };
  const issueSnapshot = async () => {
    const response = await request.get(
      `${base}/api/local/v1/fallback/issues?project_id=${project}`,
    );
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    return (body.data ?? body).issues.sort((a: any, b: any) =>
      a.id.localeCompare(b.id),
    );
  };
  const screenshot = async (name: string) => {
    const file = `${out}/${name}.png`;
    await page.screenshot({ path: file, fullPage: true });
    evidence.screenshots.push(file);
    save();
  };
  const explicitPost = async (kind: string) => {
    const entry: any = {
      kind,
      started_at: new Date().toISOString(),
      payload: { offset: 0, limit: 500, retry_failed: false },
    };
    posts.push(entry);
    save();
    const response = await request.post(base + path + "/evaluate", {
      headers: { "x-lvk-csrf": "local" },
      data: entry.payload,
    });
    entry.status = response.status();
    entry.finished_at = new Date().toISOString();
    entry.result = (await response.json()).data;
    save();
    expect(response.ok()).toBeTruthy();
    return entry.result;
  };
  const browserPosts = new Map<Request, any>();
  let browserPhase = "initial-ui";
  page.on("request", (r) => {
    if (r.method() !== "POST" || r.url() !== base + path + "/evaluate") return;
    const entry = {
      kind: browserPhase,
      started_at: new Date().toISOString(),
      payload: r.postDataJSON(),
    };
    browserPhase = "normal-ui-poll";
    posts.push(entry);
    browserPosts.set(r, entry);
    save();
  });
  page.on("response", async (r) => {
    const entry = browserPosts.get(r.request());
    if (!entry) return;
    try {
      entry.status = r.status();
      entry.result = (await r.json()).data;
      entry.finished_at = new Date().toISOString();
      save();
    } catch {
      entry.transport_error = true;
      save();
    }
  });
  // Refuse accidental different-definition launches or failed retries, without
  // changing timing, polling or responses of the permitted product requests.
  await page.route("**/saved-filters/*/evaluate", async (route) => {
    const r = route.request();
    if (
      r.url() !== base + path + "/evaluate" ||
      r.postDataJSON()?.retry_failed === true
    ) {
      await route.abort();
      throw new Error("Unexpected provider activation blocked");
    }
    await route.continue();
  });
  try {
    expect(await get()).toEqual(f);
    evidence.before = await get("/results");
    expect(
      evidence.before.state,
      "Do not relaunch an already-used fixture",
    ).toBe("unevaluated");
    const beforeIssues = await issueSnapshot();
    evidence.issues_before_sha256 = fingerprint(beforeIssues);
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto(`${base}/projects/${project}`);
    const selector = page.getByRole("combobox", {
      name: "保存フィルター",
      exact: true,
    });
    await expect(selector).toBeVisible();
    await expect(selector).toHaveValue("__none__");
    // Capture actual DOM transitions; no network delay or synthetic state.
    await page.evaluate(() => {
      const states: string[] = [];
      (window as any).__savedFilterStates = states;
      const observer = new MutationObserver(() => {
        for (const el of document.querySelectorAll(
          '[role="status"], [role="alert"]',
        )) {
          const value = el.textContent ?? "";
          if (/評価/.test(value) && states.at(-1) !== value) states.push(value);
        }
      });
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        characterData: true,
      });
    });
    evidence.launch_at = new Date().toISOString();
    save();
    const [, initial] = await Promise.all([
      selector.selectOption(f.id),
      explicitPost("initial-explicit-http"),
    ]);
    expect(initial.state).not.toBe("failed");
    await expect(
      page.getByText("評価結果はまだありません。該当0件ではありません。", {
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("status").filter({ hasText: "評価中" }),
    ).toBeVisible();
    await expect(page.getByText("表示中 0 件", { exact: true })).toHaveCount(0);
    await screenshot("semantic-evaluating-desktop-light");
    for (let i = 0; i < 270; i++) {
      const result = await get("/results");
      evidence.latest = result;
      save();
      if (result.state === "failed")
        throw new Error("Provider failed; no automatic retry permitted");
      if (result.state === "complete") break;
      await page.waitForTimeout(2000);
    }
    expect(evidence.latest.state).toBe("complete");
    expect(evidence.latest.stale).toBe(false);
    expect(evidence.latest.issue_ids).toEqual(h.semantic_plan.expected_ids);
    expect(evidence.latest.total_count).toBe(1);
    const cardIds = () =>
      page
        .locator("[data-kanban-card], [data-issue-list-row]")
        .evaluateAll((nodes) =>
          nodes
            .map(
              (n) =>
                n.getAttribute("data-kanban-card") ??
                n.getAttribute("data-issue-list-row"),
            )
            .sort(),
        );
    await expect.poll(cardIds).toEqual(h.semantic_plan.expected_ids);
    await expect(
      page.getByRole("status").filter({ hasText: "評価完了" }),
    ).toContainText("表示中 1 件");
    evidence.first_complete_at = new Date().toISOString();
    evidence.first_complete_ui_ids = await cardIds();
    evidence.dom_states = await page.evaluate(
      () => (window as any).__savedFilterStates,
    );
    await screenshot("semantic-complete-desktop-light");
    save();
    console.log(
      "FIRST_SEMANTIC_RESULT",
      JSON.stringify({
        filter: f.id,
        revision: f.revision,
        result: evidence.latest,
        publicPath,
      }),
    );
    // Preserve ordinary polling. Then verify explicit cache reuse and reopening
    // without changing the definition, Issue corpus or context.
    await expect
      .poll(
        () =>
          posts.filter(
            (p) =>
              p.kind === "normal-ui-poll" && p.result?.state === "complete",
          ).length,
        { timeout: 45000 },
      )
      .toBeGreaterThan(0);
    evidence.cache_recheck = await explicitPost("valid-cache-explicit-http");
    expect(evidence.cache_recheck).toEqual({ ...evidence.latest, limit: 500 });
    browserPhase = "reopen-ui-cache";
    await page.goto(`${base}/projects/${project}?savedFilter=${f.id}`);
    await expect.poll(cardIds).toEqual(h.semantic_plan.expected_ids);
    await expect(
      page.getByRole("status").filter({ hasText: "評価完了" }),
    ).toContainText("表示中 1 件");
    await page.emulateMedia({ colorScheme: "dark" });
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator("html")).toHaveClass(/dark/);
    await screenshot("semantic-complete-mobile-dark");
    await page
      .getByRole("button", { name: "フィルターを管理", exact: true })
      .click();
    await page.getByRole("button", { name: f.name, exact: true }).click();
    await page
      .getByRole("textbox", { name: "名前", exact: true })
      .scrollIntoViewIfNeeded();
    await screenshot("semantic-settings-mobile-dark-top");
    await page
      .getByRole("textbox", { name: "自然文の条件", exact: true })
      .scrollIntoViewIfNeeded();
    await screenshot("semantic-settings-mobile-dark-context");
    expect(await get()).toEqual(f);
    expect(await issueSnapshot()).toEqual(beforeIssues);
    evidence.unchanged_definition_and_issues = true;
    evidence.final = await get("/results");
    evidence.finished_at = new Date().toISOString();
    evidence.passed = true;
    save();
  } finally {
    evidence.ended_at = new Date().toISOString();
    save();
    console.log("SEMANTIC_EVIDENCE", publicPath);
  }
});
