import { expect, test } from "@playwright/test";

for (const mobile of [false, true]) {
  test(`validation choice survives retries and history (${mobile ? "mobile dark" : "desktop light"})`, async ({
    page,
  }) => {
    await page.setViewportSize(
      mobile ? { width: 390, height: 844 } : { width: 1280, height: 900 },
    );
    const requests: any[] = [];
    const runs: any[] = [];
    let fail = true;
    let config = { integration_skip_validation: false };
    await page.route("**/api/**", async (route) => {
      const req = route.request();
      const path = new URL(req.url()).pathname;
      const reply = (data: unknown) =>
        route.fulfill({ json: { success: true, data } });
      if (path === "/api/info") return reply({ config });
      if (path === "/api/config") {
        config = req.postDataJSON();
        return reply(config);
      }
      if (path.endsWith("/repositories"))
        return reply([{ id: "repo", display_name: "Example repo" }]);
      if (path.endsWith("/parallel-context"))
        return reply({
          activities: [
            {
              current_issue_id: "card",
              workspace_id: "workspace",
              observed_head_oid: "a".repeat(40),
              branch: "feature",
            },
          ],
          next_workspace: null,
        });
      if (path === "/api/integrations" && req.method() === "GET")
        return reply(runs);
      if (path === "/api/integrations" && req.method() === "POST") {
        const body = req.postDataJSON();
        requests.push(body);
        if (fail)
          return route.fulfill({
            status: 503,
            json: { message: "Fixture retry" },
          });
        const run = {
          id: `run-${runs.length}`,
          status: "succeeded",
          target_ref: "refs/heads/main",
          created_at: "2026-10-18",
          payload: {
            skip_validation: body.skip_validation,
            published: true,
            sources: [],
            validation: [],
            semantic_summary: "Fixture integration",
          },
        };
        runs.push(run);
        return reply(run);
      }
      return route.fulfill({
        status: 404,
        json: { message: `Unexpected: ${path}` },
      });
    });
    const open = async () => {
      await page.goto("/");
      await page.evaluate(
        (dark) => document.documentElement.classList.toggle("dark", dark),
        mobile,
      );
      await page
        .getByRole("button", { name: "Integrate / Auto Merge" })
        .click();
    };
    const select = async () => {
      await page.getByLabel("Integration repository").selectOption("repo");
      await page.getByLabel("Integration target branch").selectOption("main");
      await page.getByLabel("LVK-1: Example change").check();
      await page
        .getByLabel("Adopted Workspace for Example change")
        .selectOption("workspace");
    };
    await open();
    const skip = page.getByRole("checkbox", {
      name: /Skip all integration validation/,
    });
    await expect(skip).not.toBeChecked();
    await select();
    const start = page.getByRole("button", {
      name: "Start selected Integration",
    });
    await start.click();
    await expect(page.getByRole("alert")).toContainText("Fixture retry");
    expect(requests[0].skip_validation).toBe(false);
    await skip.click();
    await expect(skip).toBeChecked();
    await start.click();
    await expect.poll(() => requests.length).toBe(2);
    expect(requests[1].request_id).not.toBe(requests[0].request_id);
    expect(requests[1].skip_validation).toBe(true);
    fail = false;
    await expect(start).toBeEnabled();
    await start.click();
    await expect(
      page.getByText("Succeeded · Unverified", { exact: true }),
    ).toBeVisible();
    expect(requests[2]).toEqual(requests[1]);
    await expect(skip).toBeChecked();
    // A fresh page reads the persisted choice from server configuration, not local form state.
    await open();
    await expect(
      page.getByText("Succeeded · Unverified", { exact: true }),
    ).toBeVisible();
    await expect(skip).toBeChecked();
    await page.screenshot({
      path: `test-results/integration-skip/${mobile ? "mobile-dark" : "desktop-light"}.png`,
      fullPage: true,
    });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await select();
    await skip.click();
    await expect(skip).not.toBeChecked();
    await start.click();
    await expect.poll(() => requests.length).toBe(4);
    expect(requests[3].skip_validation).toBe(false);
  });
}
