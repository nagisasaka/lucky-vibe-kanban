import { test, expect } from "@playwright/test";
test("actual AppNavigation preserves only the project filter, with existing session/run navigation semantics", async ({
  page,
}) => {
  await page.goto("/navigation.html");
  const location = async () =>
    JSON.parse(await page.getByTestId("location").textContent());
  await page
    .getByRole("button", { name: "Seed ordinary", exact: true })
    .click();
  await expect(page.getByTestId("location")).toContainText("session_id");
  await page
    .getByRole("button", { name: "Baseline issue", exact: true })
    .click();
  await expect(page.getByTestId("location")).toContainText("/issues/two");
  const baseline = await location();
  await page
    .getByRole("button", { name: "Seed ordinary", exact: true })
    .click();
  await expect(page.getByTestId("location")).toContainText("agent_run_id");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect.poll(location).toEqual(baseline);
  await page
    .getByRole("button", { name: "Seed workspace", exact: true })
    .click();
  await expect(page.getByTestId("location")).toContainText("agent_run_id");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect
    .poll(location)
    .toEqual({
      ...baseline,
      search: { ...baseline.search, savedFilter: "filter" },
    });
  await page.getByRole("button", { name: "Board", exact: true }).click();
  await expect
    .poll(location)
    .toEqual({
      pathname: "/projects/project",
      search: { savedFilter: "filter" },
    });
  await page
    .getByRole("button", { name: "Other project", exact: true })
    .click();
  await expect
    .poll(location)
    .toEqual({ pathname: "/projects/other", search: {} });
  await page
    .getByRole("button", { name: "Seed workspace", exact: true })
    .click();
  await expect(page.getByTestId("location")).toContainText("savedFilter");
  await page.getByRole("button", { name: "Workspaces", exact: true }).click();
  await expect.poll(location).toEqual({ pathname: "/workspaces", search: {} });
});
