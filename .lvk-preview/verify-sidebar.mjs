// Public preview, real source/HMR. All test data and terminal transport are browser-only fixtures.
import { chromium, expect } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
const base = "https://preview2.luckyvibekanban.com";
const credentials = JSON.parse(
  await readFile(
    new URL("../.build/design-preview/credentials.json", import.meta.url),
  ),
);
const evidence = ".build/design-preview";
const results = { cases: [], consoleErrors: [], terminalConnections: 0 };
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const login = await context.request.post(base + "/api/access/basic-login", {
    headers: {
      Authorization:
        "Basic " +
        Buffer.from(credentials.username + ":" + credentials.password).toString(
          "base64",
        ),
      "x-lvk-csrf": "basic",
      Origin: base,
    },
    data: {},
  });
  if (!login.ok()) throw Error("Preview login failed");
  const projects = await (
    await context.request.get(
      base +
        "/api/board/projects?organization_id=00000000-0000-0000-0000-000000000002",
    )
  ).json();
  const project = projects.data.projects[0];
  if (!project) throw Error("Preview project unavailable");
  const page = await context.newPage();
  page.on("pageerror", (error) => results.consoleErrors.push(error.message));
  // Do not save browser preferences, edit entities, or touch real repositories during UI tests.
  await page.route("**/api/**", (route) => {
    if (
      ["GET", "HEAD"].includes(route.request().method()) ||
      route.request().url().endsWith("/provider-usage/read")
    )
      return route.fallback();
    return route.fulfill({ json: { success: true, data: null } });
  });
  let language = "EN";
  await page.route("**/api/info", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.data.config.language = language;
    body.data.config.show_release_notes = false;
    return route.fulfill({ response, json: body });
  });
  const panel = page.getByTestId("environment-panel");
  const metric = (label) =>
    panel.getByRole("button", { name: `${label} details`, exact: true });
  const terminal = page.getByRole("button", {
    name: /^(Terminal|ターミナル)$/,
  });
  const goto = async () => {
    await page.goto(base + "/projects/" + project.id);
    await panel.waitFor({ state: "attached", timeout: 60000 });
    if (!(await panel.isVisible()))
      await page
        .getByRole("button", { name: /Show Right Sidebar|右サイドバーを表示/ })
        .click();
    await expect(panel.locator("[data-pet-placeholder] img")).toBeVisible();
  };
  await goto();
  await expect(page.getByTestId("environment-hostname")).not.toHaveText("—");
  await expect(
    page.getByTestId("environment-quota-codex").getByRole("status"),
  ).not.toHaveText("Loading", { timeout: 25000 });
  await expect(
    page.getByTestId("environment-quota-claude").getByRole("status"),
  ).not.toHaveText("Loading", { timeout: 25000 });
  await page.screenshot({ path: `${evidence}/panel-live.png` });
  results.cases.push(
    "Live preview source and hostname rendered, no fabricated API data",
  );
  // Fixture source values are not written to the running backend or shared data.
  let mode = "fresh";
  const repos = [
    {
      id: "repo-one",
      name: "Application",
      path: "/isolated/fixture/application",
      wiki_enabled: false,
    },
    {
      id: "repo-two",
      name: "Library",
      path: "/isolated/fixture/library-with-a-very-long-directory-name",
      wiki_enabled: false,
    },
  ];
  let repoCount = 0;
  const known = (value) => ({ status: "known", value });
  await page.route(`**/api/projects/${project.id}/utilities/info`, (route) =>
    route.fulfill({
      json: { success: true, data: { repos: repos.slice(0, repoCount) } },
    }),
  );
  await page.route("**/api/resource-observation", (route) => {
    if (mode === "unavailable") return route.fulfill({ status: 503, json: {} });
    return route.fulfill({
      json: {
        success: true,
        data: {
          timestamp: new Date(
            Date.now() - (mode === "stale" ? 60000 : 0),
          ).toISOString(),
          system_visible: {
            cpu_usage: { percent: known(32), sampling_duration_ms: 500 },
            memory_total_bytes: known(16 * 2 ** 30),
            memory_available_bytes: known(4 * 2 ** 30),
            swap_total_bytes: known(4 * 2 ** 30),
            swap_used_bytes: known(0),
          },
          disks: [
            {
              target: "worktree_base",
              observation: known({
                total_bytes: 100 * 2 ** 30,
                used_bytes: 83 * 2 ** 30,
                used_percent: 83,
              }),
            },
          ],
        },
      },
    });
  });
  await page.route("**/api/environment-info", (route) =>
    route.fulfill({
      json: {
        success: true,
        data: {
          hostname: "fixture-build-host",
          container: true,
          uptime_seconds: null,
          running_sessions: 3,
          observed_at: new Date(
            Date.now() - (mode === "stale" ? 60000 : 0),
          ).toISOString(),
        },
      },
    }),
  );
  await page.route("**/api/provider-usage/read", (route) => {
    if (mode === "unavailable") return route.fulfill({ status: 503, json: {} });
    if (mode === "unsupported")
      return route.fulfill({
        json: { success: true, data: { result: { status: "unsupported" } } },
      });
    const now = Date.now();
    const reset = now + (mode === "expired" ? -60000 : 95 * 60000);
    const provider = route.request().postDataJSON().provider;
    const data =
      provider === "codex"
        ? {
            rateLimits: {
              limitId: "default-test",
              limitName: "Default allowance",
              primary: {
                usedPercent: 25,
                windowDurationMins: 300,
                resetsAt: reset / 1000,
              },
              secondary: {
                usedPercent: 100,
                windowDurationMins: 10080,
                resetsAt: now / 1000 + 86400,
              },
              credits: { unlimited: true },
            },
            rateLimitsByLimitId: {
              extra: {
                limitId: "extra-test",
                limitName: "Additional allowance",
                primary: {
                  usedPercent: 80,
                  windowDurationMins: 60,
                  resetsAt: now / 1000 + 3600,
                },
              },
            },
          }
        : {
            five_hour: {
              utilization: 40,
              resets_at: new Date(reset).toISOString(),
            },
            seven_day: {
              utilization: 10,
              resets_at: new Date(now + 86400000).toISOString(),
            },
            model_scoped: [
              {
                display_name: "Model-specific allowance",
                utilization: 0,
                resets_at: null,
              },
            ],
          };
    return route.fulfill({
      json: {
        success: true,
        data: {
          result: {
            quota: {
              status: "fresh",
              observedAt: new Date(
                now - (mode === "stale" ? 660000 : 0),
              ).toISOString(),
              data,
            },
          },
        },
      },
    });
  });
  await goto();
  await expect(metric("CPU")).toContainText("32%");
  await expect(metric("RAM")).toContainText("75%");
  await expect(metric("SWAP")).toContainText("0%");
  await expect(metric("DISK")).toContainText("83%");
  await expect(panel.getByTestId("environment-sessions")).toContainText(
    "3 sessions",
  );
  await expect(terminal).toBeDisabled();
  await page
    .getByRole("button", { name: "Environment details", exact: true })
    .click();
  await expect(
    page.locator('[role="dialog"][data-state="open"]'),
  ).toContainText("Docker container");
  await expect(
    page.locator('[role="dialog"][data-state="open"]'),
  ).toContainText("Last successful check");
  await expect(
    page.locator('[role="dialog"][data-state="open"]'),
  ).toContainText("Uptime");
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("button", { name: "Environment details", exact: true }),
  ).toBeFocused();
  for (const [name, used, total] of [
    ["RAM", "12 GiB", "16 GiB"],
    ["SWAP", "0 B", "4 GiB"],
    ["DISK", "83 GiB", "100 GiB"],
  ]) {
    await metric(name).click();
    await expect(
      page.locator('[role="dialog"][data-state="open"]'),
    ).toContainText(used);
    await expect(
      page.locator('[role="dialog"][data-state="open"]'),
    ).toContainText(total);
    await page.keyboard.press("Escape");
  }
  await metric("CPU").click();
  await expect(
    page.locator('[role="dialog"][data-state="open"]'),
  ).toContainText("500 ms");
  await page.keyboard.press("Escape");
  const codex = page.getByTestId("environment-quota-codex");
  const claude = page.getByTestId("environment-quota-claude");
  await expect(codex).toContainText("75%");
  await expect(codex).toContainText("0%");
  await expect(codex).toContainText("Additional allowance");
  await expect(codex).toContainText("20%");
  await expect(codex).toContainText("Waiting for reset");
  await expect(codex).toContainText("1h 35m");
  await expect(claude).toContainText("Model-specific allowance");
  await expect(claude).toContainText("100%");
  for (const provider of [codex, claude]) {
    await provider.locator("summary").click();
    await expect(provider.locator("details")).toHaveAttribute("open", "");
    await expect(provider).toContainText("Server DEFAULT account");
    await expect(provider).toContainText("Reset:");
  }
  await expect(codex).toContainText("Credits: unlimited");
  await expect(codex).toContainText("default-test");
  await expect(codex).toContainText("extra-test");
  await expect(
    codex.getByRole("button", { name: "Codex: Refresh" }),
  ).toBeDisabled();
  results.cases.push(
    "All restored resources, sessions, metadata, quota buckets, 0/100%, unlimited credits, timestamps and reset countdown; keyboard details/focus and refresh cooldown",
  );
  for (const provider of [codex, claude])
    await provider.locator("summary").click();
  for (const width of [320, 390, 768, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    for (const dark of [false, true]) {
      await page.evaluate(
        (value) => document.documentElement.classList.toggle("dark", value),
        dark,
      );
      await panel.scrollIntoViewIfNeeded();
      const metrics = page.getByTestId("environment-metrics");
      const bounds = await metrics.locator("button").evaluateAll((nodes) =>
        nodes.map((node) => {
          const r = node.getBoundingClientRect();
          return { top: r.top, left: r.left, right: r.right };
        }),
      );
      expect(
        Math.max(...bounds.map((r) => r.top)) -
          Math.min(...bounds.map((r) => r.top)),
      ).toBeLessThan(1);
      expect(bounds.every((r) => r.left >= 0 && r.right <= width)).toBe(true);
      expect(
        await panel.evaluate(
          (node) => node.scrollWidth <= node.clientWidth + 1,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `${evidence}/panel-fixture-${width}-${dark ? "dark" : "light"}.png`,
      });
    }
  }
  results.cases.push(
    "320/390/768/1440px, light/dark, resource single row and no panel horizontal overflow",
  );
  await page.setViewportSize({ width: 1440, height: 1000 });
  for (repoCount of [1, 2]) {
    await goto();
    const selector = page.getByTestId("main-terminal-repository");
    await expect(selector.locator("option")).toHaveCount(repoCount);
    await expect(terminal).toBeEnabled();
    expect(results.terminalConnections).toBe(0);
    await page
      .getByTestId("main-repository-details")
      .locator("summary")
      .click();
    for (const repo of repos.slice(0, repoCount))
      await expect(page.getByTestId("main-repository-details")).toContainText(
        repo.path,
      );
    if (repoCount === 2) {
      await selector.selectOption("repo-two");
      await expect(page.getByTestId("terminal-working-directory")).toHaveText(
        repos[1].path,
      );
    }
  }
  results.cases.push(
    "0/1/2 repositories, full main paths and selected cwd retained; no mount shell; terminal open/close uses browser-only terminal transport fixture, no real PTY",
  );
  for (mode of ["expired", "stale", "unavailable", "unsupported"]) {
    await goto();
    if (mode === "expired") await expect(codex).toContainText("Reset elapsed");
    if (mode === "stale") {
      await expect(metric("RAM")).toContainText("—");
      await expect(page.getByTestId("environment-sessions")).toContainText("—");
      await expect(codex.getByRole("status")).toHaveText("Stale");
    }
    if (mode === "unavailable") {
      await expect(metric("CPU")).toContainText("—");
      await expect(codex.getByRole("status")).toHaveText("Unavailable");
    }
    if (mode === "unsupported")
      await expect(claude.getByRole("status")).toHaveText("Unsupported");
  }
  results.cases.push(
    "Expired reset / refresh pending, stale, failed and unsupported preserve unknowns, never invent zero/full values",
  );
  mode = "unavailable";
  await goto();
  await expect(codex.getByRole("status")).toHaveText("Unavailable");
  mode = "fresh";
  const refresh = codex.getByRole("button", { name: "Codex: Refresh" });
  await expect(refresh).toBeEnabled({ timeout: 35000 });
  await refresh.click();
  await expect(codex).toContainText("75%");
  await expect(codex.getByRole("status")).toHaveText("Waiting for reset");
  results.cases.push(
    "Failed quota read recovers through real refresh/cooldown UI with isolated HTTP responses",
  );
  mode = "fresh";
  language = "JA";
  repoCount = 2;
  await goto();
  await expect(panel).toContainText("セッション");
  await expect(codex).toContainText("リセット待ち");
  await page.getByRole("button", { name: "RAM の詳細", exact: true }).click();
  await expect(
    page.locator('[role="dialog"][data-state="open"]'),
  ).toContainText("総容量");
  await page.keyboard.press("Escape");
  await expect(page.locator('[role="dialog"]')).toHaveCount(0);
  await page.screenshot({ path: `${evidence}/panel-fixture-ja.png` });
  results.cases.push("Japanese UI and tap/click accessible details");
  // Use the application's transport seam to keep terminal testing entirely in this browser.
  await page.evaluate(
    async (modulePath) => {
      const { setLocalApiTransport } = await import(modulePath);
      window.__terminalFixture = { connections: [], closed: false };
      class FixtureSocket extends EventTarget {
        readyState = WebSocket.OPEN;
        onmessage = null;
        onclose = null;
        onerror = null;
        constructor(url) {
          super();
          this.url = String(url);
          window.__terminalFixture.connections.push(this.url);
        }
        send(data) {
          if (JSON.parse(data).type === "close")
            queueMicrotask(() => {
              window.__terminalFixture.closed = true;
              this.onmessage?.(
                new MessageEvent("message", {
                  data: JSON.stringify({ type: "exit" }),
                }),
              );
            });
        }
        close() {
          if (this.readyState === WebSocket.CLOSED) return;
          this.readyState = WebSocket.CLOSED;
          const event = new CloseEvent("close");
          this.dispatchEvent(event);
          this.onclose?.(event);
        }
      }
      setLocalApiTransport({
        request: (url, init) => fetch(url, init),
        openWebSocket: (url) =>
          String(url).includes("/api/terminal/ws")
            ? new FixtureSocket(url)
            : new WebSocket(String(url).replace(/^http/, "ws")),
      });
    },
    "/@fs" +
      process.cwd() +
      "/packages/web-core/src/shared/lib/localApiTransport.ts",
  );
  const selector = page.getByTestId("main-terminal-repository");
  await selector.selectOption("repo-two");
  expect(results.terminalConnections).toBe(0);
  await terminal.click();
  await expect(terminal).toHaveAttribute("aria-expanded", "true");
  await expect(selector).toBeDisabled();
  await expect
    .poll(() =>
      page.evaluate(() => window.__terminalFixture.connections.length),
    )
    .toBe(1);
  const transportUrl = await page.evaluate(
    () => window.__terminalFixture.connections[0],
  );
  results.terminalConnections = 1;
  expect(new URL(transportUrl).searchParams.get("repo_id")).toBe("repo-two");
  await terminal.click();
  await expect
    .poll(() => page.evaluate(() => window.__terminalFixture.closed))
    .toBe(true);
  await expect(terminal).toHaveAttribute("aria-expanded", "false");
  await expect(selector).toBeEnabled();
  await page.evaluate(
    async (modulePath) => {
      const { setLocalApiTransport } = await import(modulePath);
      setLocalApiTransport(null);
    },
    "/@fs" +
      process.cwd() +
      "/packages/web-core/src/shared/lib/localApiTransport.ts",
  );
  if (results.consoleErrors.length)
    console.log("First browser error", results.consoleErrors[0]);
  expect(results.consoleErrors).toEqual([]);
  console.log(JSON.stringify(results, null, 2));
  await writeFile(
    `${evidence}/sidebar-results.json`,
    JSON.stringify(results, null, 2) + "\n",
  );
} finally {
  await browser.close();
}
