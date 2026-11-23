import { test, expect } from "@playwright/test";
test("edits shared machine and semantic conditions with clear direction and bounds", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Todo", { exact: true }).check();
  await page.getByLabel("In Progress", { exact: true }).check();
  await page.getByLabel("Alice", { exact: true }).check();
  await page.getByLabel("bug", { exact: true }).check();
  await page.getByLabel("release", { exact: true }).check();
  await page.getByLabel("タグの一致方法").selectOption("all");
  await page.getByLabel("作成期間", { exact: true }).selectOption("relative");
  await page.getByLabel("直近の秒数").fill("3600");
  await page.getByLabel("更新期間", { exact: true }).selectOption("absolute");
  await page.getByLabel("開始", { exact: true }).fill("2026-11-01T00:00");
  await page
    .getByLabel("対象Issueとの関係", { exact: true })
    .selectOption("blocking");
  await page.getByLabel("対象Issue", { exact: true }).selectOption("release");
  await page.getByLabel("Issueのアーカイブ範囲").selectOption("all");
  const definition = JSON.parse(
    await page.getByTestId("definition").textContent(),
  );
  expect(definition.machine).toMatchObject({
    status_ids: ["todo", "progress"],
    assignee_ids: ["alice"],
    tag_ids: ["bug", "release"],
    tag_mode: "all",
    created_at: { kind: "relative", seconds: 3600 },
    updated_at: { kind: "absolute" },
    relation: { kind: "blocking", target_issue_id: "release" },
    archived: "all",
  });
  expect(definition.context).toContain("v2");
  await expect(page.getByText("対象をブロックするIssue（1段階）")).toHaveCount(
    1,
  );
});
test("pending/failure do not claim zero results, stale and complete are distinct", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("status")).toContainText("未評価");
  await expect(page.getByRole("status")).toContainText("該当0件ではありません");
  await page.getByLabel("評価状態").selectOption("failed");
  await expect(page.getByRole("alert")).toContainText("該当0件ではありません");
  await page.getByLabel("前回結果").check();
  await expect(page.getByRole("alert")).toContainText("古い結果");
  await expect(page.getByRole("alert")).toContainText("表示中 2 件");
  await page.getByLabel("評価状態").selectOption("evaluating");
  await expect(page.getByRole("status")).toContainText("結果を更新中");
});
test("desktop and narrow dark/light presentations", async ({ page }) => {
  await page.goto("/");
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.screenshot({
    path: ".build/saved-filters/desktop.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({
    path: ".build/saved-filters/mobile-dark.png",
    fullPage: true,
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
