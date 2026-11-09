import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

// Opt-in only: the runner must provide its own synthetic repositories and DB.
const fixturePath = process.env.LVK_E2E349_FIXTURE;
const fixture = fixturePath
  ? JSON.parse(readFileSync(fixturePath, "utf8"))
  : null;
test.skip(!fixture, "Requires the isolated LOCAL-349 fixture runner");
test.beforeEach(() => {
  expect(fixture.owner.purpose).toBe("LOCAL-349 isolated UI acceptance");
  expect(fixture.instance).toBe(process.env.LVK_INSTANCE_DIR);
});

const issueUrl = () =>
  `/projects/${fixture.project.id}/issues/${fixture.issue.id}`;
const chooser = (page: Page, title: string) =>
  page.getByRole("dialog").filter({
    has: page.getByRole("heading", { name: title, exact: true }),
  });

test("product target dialog keeps one preferred Repo, replaces selection and confirms one; normal mode keeps all presets (component UI driver)", async ({
  page,
}) => {
  await page.goto("/workspaces");
  await expect(
    page.getByRole("button", { name: "New workspace", exact: true }),
  ).toBeVisible();
  const open = async (singleRepository: boolean, preferred = false) => {
    await page.evaluate(
      ({ asset, props }) => {
        const win = window as unknown as { local349DialogResult?: unknown };
        win.local349DialogResult = null;
        void import(asset)
          .then((module) => {
            const dialog = Object.values(module).find(
              (value) =>
                typeof (value as { show?: unknown })?.show === "function",
            ) as { show: (props: unknown) => Promise<unknown> };
            return dialog.show(props);
          })
          .then((result) => {
            win.local349DialogResult = result;
          });
      },
      {
        asset: fixture.dialog_asset,
        props: {
          projectId: fixture.project.id,
          singleRepository,
          allowedModes: ["worktree"],
          initialRepos: preferred
            ? [{ repo_id: fixture.repos[0].id, target_branch: "main" }]
            : undefined,
          title: "LOCAL349 component target",
        },
      },
    );
  };
  const dialog = chooser(page, "LOCAL349 component target");
  await open(true);
  await expect(dialog.getByRole("checkbox", { checked: true })).toHaveCount(1);
  await expect(
    dialog.getByRole("checkbox", { name: "LOCAL349 Repo B" }),
  ).toBeChecked();
  await dialog.getByRole("checkbox", { name: "LOCAL349 Repo A" }).check();
  await expect(dialog.getByRole("checkbox", { checked: true })).toHaveCount(1);
  await dialog
    .getByRole("button", { name: "Use this workspace", exact: true })
    .click();
  const result = await page.evaluate(
    () =>
      (
        window as unknown as {
          local349DialogResult: {
            selection: { repos: Array<{ repo: { id: string } }> };
          };
        }
      ).local349DialogResult,
  );
  expect(result.selection.repos.map((r) => r.repo.id)).toEqual([
    fixture.repos[0].id,
  ]);
  await open(true, true);
  await expect(
    dialog.getByRole("checkbox", { name: "LOCAL349 Repo A" }),
  ).toBeChecked();
  await expect(dialog.getByRole("checkbox", { checked: true })).toHaveCount(1);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await open(false);
  await expect(dialog.getByRole("checkbox", { checked: true })).toHaveCount(2);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await open(false);
  await expect(dialog.getByRole("checkbox", { checked: true })).toHaveCount(2);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
});

async function openProjectSettings(page: Page) {
  await page.goto(issueUrl());
  await page
    .getByRole("button", { name: "Settings", exact: true })
    .first()
    .click();
  const settings = page.getByRole("dialog", { name: "Settings", exact: true });
  await settings
    .getByRole("navigation")
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  await settings.getByText(fixture.project.name, { exact: true }).click();
  await settings
    .getByRole("button", { name: /Local execution configuration/ })
    .click();
  const editor = settings
    .getByText("Setup Script", { exact: true })
    .locator("..")
    .locator("textarea");
  await editor.fill("echo LOCAL349 unsaved synthetic setup");
  return { settings, editor };
}

for (const removeOtherFirst of [false, true]) {
  test(`preset removal protects dirty editor (${removeOtherFirst ? "last Repo" : "next Repo"})`, async ({
    page,
    request,
  }) => {
    const { settings, editor } = await openProjectSettings(page);
    const row = (path: string) =>
      settings
        .getByText(path, { exact: true })
        .locator("..")
        .filter({
          has: page.getByRole("button", { name: "Remove from preset" }),
        });
    if (removeOtherFirst) {
      let dialogs = 0;
      const unexpected = async (d: import("@playwright/test").Dialog) => {
        dialogs++;
        await d.dismiss();
      };
      page.on("dialog", unexpected);
      await row(fixture.repos[0].path)
        .getByRole("button", { name: "Remove from preset" })
        .click();
      await expect(row(fixture.repos[0].path)).toHaveCount(0);
      page.off("dialog", unexpected);
      expect(dialogs).toBe(0);
      await expect(editor).toHaveValue("echo LOCAL349 unsaved synthetic setup");
    }
    page.once("dialog", async (d) => {
      expect(d.message()).toContain("Discard unsaved changes");
      await d.dismiss();
    });
    await row(fixture.repos[1].path)
      .getByRole("button", { name: "Remove from preset" })
      .click();
    await expect(row(fixture.repos[1].path)).toBeVisible();
    await expect(editor).toHaveValue("echo LOCAL349 unsaved synthetic setup");
    page.once("dialog", async (d) => d.accept());
    await row(fixture.repos[1].path)
      .getByRole("button", { name: "Remove from preset" })
      .click();
    await expect(row(fixture.repos[1].path)).toHaveCount(0);
    if (removeOtherFirst) {
      await expect(
        settings.getByText("No repositories registered", { exact: true }),
      ).toBeVisible();
    } else {
      await expect(row(fixture.repos[0].path)).toBeVisible();
      await expect(
        settings
          .getByText("Setup Script", { exact: true })
          .locator("..")
          .locator("textarea"),
      ).toHaveValue("");
    }
    const savedRepo = (
      await (await request.get(`/api/repos/${fixture.repos[1].id}`)).json()
    ).data;
    expect(savedRepo.setup_script).toBeNull();
  });
}
