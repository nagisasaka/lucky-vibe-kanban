import { chromium, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
const base = "https://preview2.luckyvibekanban.com";
const credentials = JSON.parse(
  await readFile(
    new URL("../.build/design-preview/credentials.json", import.meta.url),
  ),
);
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
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
  const sockets = [];
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/terminal")) sockets.push(socket.url());
  });
  await page.route(`**/api/projects/${project.id}/utilities/info`, (route) =>
    route.fulfill({
      json: { success: true, data: { repos: [] } },
    }),
  );
  await page.goto(base + "/projects/" + project.id);
  const panel = page.getByTestId("environment-panel");
  await panel.waitFor({ state: "attached", timeout: 60000 });
  if (!(await panel.isVisible())) {
    await page
      .getByRole("button", { name: /Show Right Sidebar|右サイドバーを表示/ })
      .click();
  }
  await expect(panel.locator("[data-pet-placeholder] img")).toBeVisible();
  const host = await (
    await context.request.get(base + "/api/environment-info")
  ).json();
  await expect(page.getByTestId("environment-hostname")).toHaveText(
    host.data.hostname,
  );
  const terminal = page.getByRole("button", {
    name: /^(Terminal|ターミナル)$/,
  });
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal).toBeVisible();
  await expect(terminal).toBeDisabled();
  for (const width of [1440, 360]) {
    await page.setViewportSize({ width, height: 900 });
    for (const dark of [false, true]) {
      await page.evaluate(
        (value) => document.documentElement.classList.toggle("dark", value),
        dark,
      );
      await panel.scrollIntoViewIfNeeded();
      await expect(panel.locator("[data-pet-placeholder] img")).toBeVisible();
      const metrics = await panel
        .locator("dl")
        .first()
        .locator("dt")
        .allTextContents();
      expect(metrics).toEqual(["CPU", "RAM", "SWAP", "DISK"]);
      const positions = await panel
        .locator("dl")
        .first()
        .locator("dt")
        .evaluateAll((nodes) =>
          nodes.map((node) => node.getBoundingClientRect().top),
        );
      expect(Math.max(...positions) - Math.min(...positions)).toBeLessThan(1);
      await page.screenshot({
        path: `.build/design-preview/sidebar-${width}-${dark ? "dark" : "light"}.png`,
      });
    }
  }
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route(`**/api/projects/${project.id}/utilities/info`, (route) =>
    route.fulfill({
      json: {
        success: true,
        data: {
          repos: [
            {
              id: "sidebar-regression-repo",
              name: "Sidebar regression",
              path: "/unused-sidebar-fixture",
              wiki_enabled: false,
            },
          ],
        },
      },
    }),
  );
  await page.reload();
  await expect(page.getByTestId("main-terminal-repository")).toBeAttached({
    timeout: 60000,
  });
  if (!(await panel.isVisible()))
    await page
      .getByRole("button", { name: /Show Right Sidebar|右サイドバーを表示/ })
      .click();
  await expect(terminal).toBeEnabled();
  expect(sockets).toHaveLength(0);
  console.log(
    "Pet placeholder and hostname restored; metrics remain one row in light/dark at 360/1440px; terminal stays visible without a repo and enabled with one; no shell starts on mount",
  );
} finally {
  await browser.close();
}
