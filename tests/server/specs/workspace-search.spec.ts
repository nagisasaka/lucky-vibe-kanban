import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve, join } from "node:path";

// Real /workspaces route, Sidebar, sync and list APIs against a disposable DB.
// Only fixtures and one deliberately failed related-info fetch are synthetic.
test("Issue identifier search preserves filters, pagination and ordinary refresh", async ({
  page,
  request,
}, testInfo) => {
  test.skip(
    process.env.LVK_E2E_SEARCH !== "1",
    "Requires the isolated search runner",
  );
  const instance = process.env.LVK_INSTANCE_DIR!;
  expect(instance.startsWith(resolve(".build/search-instance-"))).toBeTruthy();
  expect(
    readFileSync(join(instance, "search-fixture-owner"), "utf8"),
  ).toContain("LOCAL-162");
  const base = new URL(testInfo.project.use.baseURL!);
  expect(base.hostname).toBe("127.0.0.1");
  expect(["4020", "8443", "8444"]).not.toContain(base.port);
  const seed = (mode: string) =>
    JSON.parse(
      execFileSync(
        "python3",
        [resolve("tests/server/fixtures/workspace-search.py"), instance, mode],
        { encoding: "utf8" },
      ),
    );
  const ids = seed("create");
  const access = await (await request.get("/api/access/status")).json();
  const info = (await (await request.get("/api/info")).json()).data;
  expect(
    (
      await request.put("/api/config", {
        headers: { "x-lvk-csrf": access.csrf },
        data: {
          ...info.config,
          onboarding_acknowledged: true,
          disclaimer_acknowledged: true,
          language: "EN",
          show_release_notes: false,
          analytics_enabled: false,
          showcases: { seen_features: ["workspaces-guide"] },
        },
      })
    ).ok(),
  ).toBeTruthy();
  const related = async (suffix: string) => {
    const response = await request.get(`/api/local/v1/fallback/${suffix}`);
    expect(response.ok(), await response.text()).toBeTruthy();
    return (await response.json()).workspaces;
  };
  const userRows = await related("user_workspaces");
  expect(userRows).toHaveLength(6);
  expect(
    userRows.filter(
      (row: { issue_simple_id: string }) => row.issue_simple_id === "LOCAL-106",
    ),
  ).toHaveLength(4);
  expect(
    await related(`project_workspaces?project_id=${ids.project}`),
  ).toHaveLength(5);
  for (const row of await related(
    `project_workspaces?project_id=${ids.project}`,
  )) {
    expect(row.project_id).toBe(ids.project);
    expect(row.issue_simple_id).toMatch(/^LOCAL-(106|1060|107)$/);
  }
  expect(await related(`user_workspaces?user_id=${ids.otherUser}`)).toEqual([]);
  expect(
    await related(`project_workspaces?project_id=${ids.otherUser}`),
  ).toEqual([]);

  let failRelated = true;
  let relatedRequests = 0;
  await page.route(
    "**/api/local/v1/fallback/user_workspaces*",
    async (route) => {
      relatedRequests++;
      if (failRelated)
        await route.fulfill({ status: 503, body: "fixture unavailable" });
      else await route.continue();
    },
  );
  await page.goto("/workspaces");
  const search = page.getByPlaceholder("Search...", { exact: true });
  const name = (value: string) => page.getByText(value, { exact: true });
  await expect(search).toBeVisible();
  await expect.poll(() => relatedRequests).toBeGreaterThan(0);
  await search.fill("Legacy");
  await expect(name("Legacy standalone")).toBeVisible();
  await search.fill("feature/solo");
  await expect(name("Legacy standalone")).toBeVisible();
  await search.fill("106");
  await expect(name("Alpha target")).toHaveCount(0);
  failRelated = false;
  // Wait for the existing 30s polling path; no new refresh mechanism or reload.
  await expect(name("Alpha target")).toBeVisible({ timeout: 40_000 });
  await expect(name("Beta sibling")).toBeVisible();
  await expect(name("Gamma other project")).toBeVisible();
  await expect(name("Delta longer")).toBeVisible();
  await expect(name("Epsilon different")).toHaveCount(0);
  await expect(name("Archive target")).toHaveCount(0);
  for (const query of ["LOCAL-106", "  lOcAl-106  ", "  106  "]) {
    await search.fill(query);
    await expect(name("Alpha target")).toBeVisible();
    await expect(name("Delta longer")).toBeVisible();
  }
  await search.fill("107");
  await expect(name("Epsilon different")).toBeVisible();
  await expect(name("Alpha target")).toHaveCount(0);
  await search.fill("1060");
  await expect(name("Delta longer")).toBeVisible();
  await expect(name("Alpha target")).toHaveCount(0);
  await search.fill("   ");
  await expect(name("Pad 000")).toBeVisible();
  await expect(name("Alpha target")).toHaveCount(0); // beyond the ordinary 50-row display limit
  await search.fill("106");
  await expect(name("Alpha target")).toBeVisible();

  await page.getByRole("button", { name: /View Archive/ }).click();
  await expect(name("Archive target")).toBeVisible();
  await expect(name("Alpha target")).toHaveCount(0);
  await search.fill("107");
  await expect(name("Archive target")).toHaveCount(0);
  await search.fill("106");
  await page.getByRole("button", { name: "Back to Active" }).click();

  await page
    .getByRole("button", { name: "Filter workspaces", exact: true })
    .click();
  await page.getByRole("button", { name: "Project", exact: true }).click();
  await page.getByRole("menuitemcheckbox", { name: "Project One" }).click();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(name("Alpha target")).toBeVisible();
  await expect(name("Gamma other project")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Filter workspaces", exact: true })
    .click();
  await page.getByRole("button", { name: /PR/ }).click();
  await page.getByRole("menuitem", { name: "Has PR", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(name("Alpha target")).toHaveCount(0);
  await page.getByRole("button", { name: /View Archive/ }).click();
  await expect(name("Archive target")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Filter workspaces", exact: true })
    .click();
  await page.getByRole("button", { name: /PR/ }).click();
  await page.getByRole("menuitem", { name: "No PR", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(name("Archive target")).toBeVisible();
  await page.getByRole("button", { name: "Back to Active" }).click();
  await expect(name("Alpha target")).toBeVisible();
  await expect(name("Gamma other project")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Filter workspaces", exact: true })
    .click();
  await page.getByRole("button", { name: "Clear filters" }).click();
  await page.keyboard.press("Escape");
  await expect(name("Alpha target")).toBeVisible();
  await expect(name("Gamma other project")).toBeVisible();

  seed("update");
  await expect(name("Alpha target")).toHaveCount(0, { timeout: 40_000 });
  await expect(name("Beta sibling")).toHaveCount(0);
  await expect(name("Gamma other project")).toBeVisible();
  await search.fill("108");
  await expect(name("Alpha target")).toBeVisible();
  await expect(name("Beta sibling")).toBeVisible();
  await page.getByRole("button", { name: /View Archive/ }).click();
  await expect(name("Archive target")).toBeVisible();
  await search.fill("106");
  await expect(name("Archive target")).toHaveCount(0);
  await search.fill("  LoCaL-108  ");
  await expect(name("Archive target")).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("issue-search.png"),
    fullPage: true,
  });
});
