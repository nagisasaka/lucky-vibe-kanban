import { test, expect } from "@playwright/test";
const machine = {
  search: null,
  status_ids: [],
  priorities: [],
  assignee_ids: [],
  tag_ids: [],
  tag_mode: "any",
  archived: "all",
  created_at: null,
  updated_at: null,
  relation: null,
};
const definition = {
  id: "11111111-1111-4111-8111-111111111111",
  project_id: "project",
  name: "Release",
  machine,
  natural_language: null,
  context: null,
  revision: 1,
  created_at: "2026-11-01",
  updated_at: "2026-11-01",
};
test("selected filter alone evaluates; all archived IDs, evidence updates, invalid IDs and clear", async ({
  page,
}) => {
  let evaluations = 0;
  let ids = ["active", "archived"];
  await page.route("**/api/projects/project/saved-filters**", async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname.endsWith("/evaluate")) {
      evaluations++;
      await route.fulfill({
        json: {
          success: true,
          data: {
            saved_filter_id: "11111111-1111-4111-8111-111111111111",
            project_id: "project",
            state: "complete",
            issue_ids: ids,
            total_count: ids.length,
            offset: 0,
            limit: 500,
            next_offset: null,
            stale: false,
            error: null,
            definition_revision: 1,
          },
        },
      });
    } else if (url.pathname.endsWith("/22222222-2222-4222-8222-222222222222"))
      await route.fulfill({
        status: 404,
        json: { success: false, message: "Filter not found" },
      });
    else if (url.pathname.endsWith("/11111111-1111-4111-8111-111111111111"))
      await route.fulfill({ json: { success: true, data: definition } });
    else
      await route.fulfill({
        json: {
          success: true,
          data: { saved_filters: [definition], next_offset: null },
        },
      });
  });
  await page.goto("/?integration");
  await expect(
    page.getByRole("button", { name: "Release", exact: true }),
  ).toBeVisible();
  expect(evaluations).toBe(0);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.getByTestId("result")).toHaveText("active,archived");
  await expect(page).toHaveURL(
    /savedFilter=11111111-1111-4111-8111-111111111111/,
  );
  ids = ["archived"];
  await page.getByRole("button", { name: "Change issue", exact: true }).click();
  await expect(page.getByTestId("result")).toHaveText("archived");
  await page.getByRole("button", { name: "Other", exact: true }).click();
  await expect(page.getByTestId("result")).toHaveText("pending");
  await expect(page.getByRole("alert")).toContainText("適用できません");
  await page.getByRole("button", { name: "Clear", exact: true }).click();
  await expect(page.getByTestId("selected")).toHaveText("");
  const before = evaluations;
  await page.getByRole("button", { name: "Change issue", exact: true }).click();
  await page.waitForTimeout(600);
  expect(evaluations).toBe(before);
});
test("create, rename and edit keep the stable id and send only the definition", async ({
  page,
}) => {
  const definitions: any[] = [];
  let current: any = { ...definition };
  await page.route("**/api/projects/project/saved-filters**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (
      (request.method() === "POST" && !url.pathname.endsWith("/evaluate")) ||
      request.method() === "PUT"
    ) {
      const body = request.postDataJSON();
      definitions.push(body);
      current = {
        ...current,
        ...body.definition,
        revision: current.revision + 1,
      };
      await route.fulfill({ json: { success: true, data: current } });
    } else if (url.pathname.endsWith("/11111111-1111-4111-8111-111111111111"))
      await route.fulfill({ json: { success: true, data: current } });
    else
      await route.fulfill({
        json: {
          success: true,
          data: { saved_filters: [current], next_offset: null },
        },
      });
  });
  await page.goto("/?integration");
  await page
    .getByRole("button", { name: "フィルターを作成", exact: true })
    .click();
  await page.getByLabel("名前", { exact: true }).fill("New");
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "New", exact: true }),
  ).toBeVisible();
  await page.getByLabel("名前", { exact: true }).fill("Renamed");
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Renamed", exact: true }),
  ).toBeVisible();
  expect(definitions).toHaveLength(2);
  expect(definitions[1].expected_revision).toBe(2);
  expect(Object.keys(definitions[1].definition).sort()).toEqual([
    "context",
    "machine",
    "name",
    "natural_language",
  ]);
  await expect(
    page.getByRole("link", { name: "カンバンを開く", exact: true }),
  ).toHaveAttribute(
    "href",
    "/projects/project?savedFilter=11111111-1111-4111-8111-111111111111",
  );
});
test("stable-ID deep link uses current definition and marks retained results after evaluation failure", async ({
  page,
}) => {
  let failed = false;
  await page.route("**/api/projects/project/saved-filters**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/evaluate"))
      await route.fulfill(
        failed
          ? { status: 503, json: { success: false, message: "offline" } }
          : {
              json: {
                success: true,
                data: {
                  saved_filter_id: "11111111-1111-4111-8111-111111111111",
                  project_id: "project",
                  state: "complete",
                  issue_ids: ["archived"],
                  total_count: 1,
                  offset: 0,
                  limit: 500,
                  next_offset: null,
                  stale: false,
                  error: null,
                  definition_revision: 7,
                },
              },
            },
      );
    else if (path.endsWith("/11111111-1111-4111-8111-111111111111"))
      await route.fulfill({
        json: {
          success: true,
          data: { ...definition, name: "Current name", revision: 7 },
        },
      });
    else
      await route.fulfill({
        json: {
          success: true,
          data: {
            saved_filters: [
              { ...definition, name: "Current name", revision: 7 },
            ],
            next_offset: null,
          },
        },
      });
  });
  await page.goto(
    "/?integration&savedFilter=11111111-1111-4111-8111-111111111111",
  );
  await expect(page.getByTestId("selected")).toHaveText(
    "11111111-1111-4111-8111-111111111111",
  );
  await expect(page.getByTestId("result")).toHaveText("archived");
  await expect(
    page.getByRole("button", { name: "Current name", exact: true }),
  ).toBeVisible();
  failed = true;
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("評価失敗");
  await expect(page.getByRole("alert")).toContainText("古い結果");
  await expect(page.getByTestId("result")).toHaveText("archived");
});
test("an unconfirmed save retry reuses request ID and deletion only deletes the filter", async ({
  page,
}) => {
  let current: any = { ...definition };
  let fail = true;
  let requests: any[] = [];
  let deleted: any;
  await page.route("**/api/projects/project/saved-filters**", async (route) => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    if (req.method() === "PUT") {
      requests.push(req.postDataJSON());
      if (fail) {
        fail = false;
        await route.fulfill({
          status: 503,
          json: { success: false, message: "unknown" },
        });
      } else {
        current = { ...current, ...req.postDataJSON().definition, revision: 2 };
        await route.fulfill({ json: { success: true, data: current } });
      }
    } else if (req.method() === "DELETE") {
      deleted = req.postDataJSON();
      current = null;
      await route.fulfill({ json: { success: true, data: null } });
    } else if (path.endsWith("/11111111-1111-4111-8111-111111111111"))
      await route.fulfill({ json: { success: true, data: current } });
    else
      await route.fulfill({
        json: {
          success: true,
          data: { saved_filters: current ? [current] : [], next_offset: null },
        },
      });
  });
  await page.goto("/?integration");
  await page.getByRole("button", { name: "Release", exact: true }).click();
  await page.getByLabel("名前", { exact: true }).fill("Retry");
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("unknown");
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Retry", exact: true }),
  ).toBeVisible();
  expect(requests).toHaveLength(2);
  expect(requests[0]).toEqual(requests[1]);
  await page
    .getByRole("button", { name: "フィルターを削除", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "フィルターを削除", exact: true })
    .click();
  await expect(
    page.getByText("保存フィルターはまだありません。"),
  ).toBeVisible();
  expect(deleted.expected_revision).toBe(2);
  expect(deleted.request_id).toBeTruthy();
});

test("malformed stable ID is explained without applying another resource", async ({
  page,
}) => {
  const requests: string[] = [];
  await page.route("**/api/projects/project/saved-filters**", async (route) => {
    requests.push(route.request().url());
    await route.fulfill({
      json: { success: true, data: { saved_filters: [], next_offset: null } },
    });
  });
  await page.goto("/?integration&savedFilter=..");
  await expect(page.getByRole("alert")).toContainText("適用できません");
  await expect(page.getByTestId("result")).toHaveText("pending");
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  expect(
    requests.every((url) => new URL(url).pathname.endsWith("/saved-filters")),
  ).toBe(true);
});
