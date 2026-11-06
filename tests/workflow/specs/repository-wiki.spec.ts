import { expect, test, type Page } from "@playwright/test";

async function fixture(page: Page) {
  const state = {
    enabled: true,
    status: "current",
    wiki_exists: true,
    target_branch: "main",
    output_language: "en",
    last_success: "2026-11-06T00:00:00Z",
    active_run_id: null,
    bootstrap: null as {
      workflow_run_id: string;
      phase: string;
      child: null;
    } | null,
    file_operation: null,
    maintenance_workspace_id: null,
    coding_errors: [],
  };
  const files = {
    repository_id: "wiki-fixture",
    branch: "main",
    source: "a".repeat(40),
    files: [
      {
        path: "openwiki/INSTRUCTIONS.md",
        content: "Repository-specific instructions",
        default_content: "LVK default instructions",
        exists: true,
      },
      {
        path: ".openwikiignore",
        content: "# existing exclusions",
        default_content: "",
        exists: true,
      },
    ],
  };
  const mutations: Array<{ path: string; body: Record<string, string> }> = [];
  let fail = false;
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (request.method() === "GET" && path.includes("/utilities/wiki")) {
      await route.fulfill({
        json: {
          success: true,
          data: {
            workspace_id: "wiki-project",
            repo_id: "wiki-fixture",
            repo_name: "fixture",
            repo_display_name: "Fixture",
            source: "main_repository",
            source_links: [],
            wiki: {
              exists: state.wiki_exists,
              config: null,
              index: state.wiki_exists
                ? {
                    path: "index.md",
                    metadata: null,
                    content: "Published Wiki",
                  }
                : null,
              pages: [],
            },
          },
        },
      });
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill({
        json: { success: true, data: path.endsWith("/files") ? files : state },
      });
      return;
    }
    const body = request.postDataJSON();
    mutations.push({ path, body });
    if (fail) {
      await route.fulfill({
        status: 409,
        json: { success: false, message: "Fixture source conflict" },
      });
      return;
    }
    if (path.endsWith("/files")) {
      const file = files.files.find((file) => file.path === body.path)!;
      file.content = body.content;
      file.exists = true;
      files.source = "b".repeat(40);
    } else if (path.endsWith("/delete")) {
      state.wiki_exists = false;
      state.status = "uninitialized";
    } else if (path.endsWith("/rebuild")) {
      state.status = "initializing";
      state.bootstrap = {
        workflow_run_id: "running-fixture",
        phase: "generating",
        child: null,
      };
    }
    await route.fulfill({
      json: { success: true, data: path.endsWith("/files") ? files : state },
    });
  });
  await page.goto("/?mode=repository-wiki");
  await page.evaluate(() =>
    (
      window as unknown as { __setUiLanguage: (s: string) => Promise<void> }
    ).__setUiLanguage("en"),
  );
  await expect(
    page.getByLabel("openwiki/INSTRUCTIONS.md", { exact: true }),
  ).toHaveValue("Repository-specific instructions");
  return {
    mutations,
    fail: () => {
      fail = true;
    },
    succeed: () => {
      fail = false;
    },
  };
}

test("both real files use the same draft/reset/save contract without generation", async ({
  page,
}) => {
  const { mutations } = await fixture(page);
  const instructions = page.getByLabel("openwiki/INSTRUCTIONS.md", {
    exact: true,
  });
  await expect(instructions).toHaveJSProperty("readOnly", true);
  await page
    .getByRole("button", { name: "Restore LVK default in editor", exact: true })
    .first()
    .click();
  await expect(instructions).toHaveValue("LVK default instructions");
  expect(mutations).toEqual([]);
  await instructions.fill("New saved instructions");
  await page
    .getByRole("button", { name: "Save settings", exact: true })
    .nth(1)
    .click();
  await expect.poll(() => mutations.length).toBe(1);
  expect(mutations[0].body).toMatchObject({
    path: "openwiki/INSTRUCTIONS.md",
    content: "New saved instructions",
    source: "a".repeat(40),
  });
  await page
    .getByRole("button", { name: "Restore LVK default in editor", exact: true })
    .nth(1)
    .click();
  const ignore = page.getByLabel(".openwikiignore", { exact: true });
  await expect(ignore).toHaveValue("");
  await ignore.fill("# comment\nprivate/");
  await page
    .getByRole("button", { name: "Save settings", exact: true })
    .nth(2)
    .click();
  await expect.poll(() => mutations.length).toBe(2);
  expect(mutations[1].body).toMatchObject({
    path: ".openwikiignore",
    content: "# comment\nprivate/",
    source: "b".repeat(40),
  });
  expect(mutations.every((m) => m.path.endsWith("/files"))).toBe(true);
});

test("deletion requires full target and preserves published state on conflict", async ({
  page,
}) => {
  const api = await fixture(page);
  const button = page.getByRole("button", {
    name: "Delete generated Wiki",
    exact: true,
  });
  await expect(button).toBeDisabled();
  const confirmation = page.getByLabel(
    "Type fixture/openwiki exactly to delete",
  );
  await confirmation.fill("other/openwiki");
  await expect(button).toBeDisabled();
  await confirmation.fill("fixture/openwiki");
  api.fail();
  await button.click();
  await expect(page.getByRole("alert")).toContainText(
    "Fixture source conflict",
  );
  await expect(button).toBeVisible();
  api.succeed();
  await button.click();
  await expect(
    page.getByRole("button", { name: "Initialize Wiki", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("canonical-wiki-state")).toHaveText(
    "uninitialized",
  );
  expect(api.mutations.every((m) => m.path.endsWith("/delete"))).toBe(true);
  await page
    .getByRole("button", { name: "Initialize Wiki", exact: true })
    .click();
  await expect.poll(() => api.mutations.at(-1)?.path).toContain("/sync");
  expect(api.mutations.at(-1)?.body.initialize).toBe(true);
});

test("rebuild uses saved source and remains a running operation until publication", async ({
  page,
}) => {
  const api = await fixture(page);
  await page.getByRole("button", { name: "Edit", exact: true }).first().click();
  await page
    .getByLabel("openwiki/INSTRUCTIONS.md", { exact: true })
    .fill("unsaved instructions");
  const button = page.getByRole("button", {
    name: "Rebuild using existing Wiki",
    exact: true,
  });
  await expect(button).toBeDisabled();
  await page
    .getByLabel("openwiki/INSTRUCTIONS.md", { exact: true })
    .fill("Repository-specific instructions");
  api.fail();
  await button.click();
  await expect(page.getByRole("alert")).toContainText(
    "Fixture source conflict",
  );
  await expect(button).toBeVisible();
  api.succeed();
  await button.click();
  await expect(page.getByRole("status").first()).toContainText("Initializing");
  await expect(button).toBeDisabled();
  await expect(page.getByTestId("canonical-wiki-state")).toHaveText(
    "published",
  );
  expect(api.mutations[1].body).toMatchObject({
    branch: "main",
    source: "a".repeat(40),
  });
  await expect(
    page.getByText("The current Wiki stays published", { exact: false }),
  ).toBeVisible();
});
