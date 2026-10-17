import { expect, test } from "@playwright/test";

test("token registration, rotation, permission error and removal never reveal saved credentials", async ({
  page,
}) => {
  const entries = [
    { repository: "owner/first", configured: false },
    { repository: "owner/second", configured: false },
  ];
  const saves: Array<{ repository: string; token: string }> = [];
  let denyCheck = false;
  await page.route(
    "**/api/repos/test-repo/github-credentials{,/check}",
    async (route) => {
      const request = route.request();
      const body = request.method() === "GET" ? null : request.postDataJSON();
      if (request.method() === "PUT") {
        saves.push(body);
        entries.find((e) => e.repository === body.repository)!.configured =
          true;
      }
      if (request.method() === "DELETE")
        entries.find((e) => e.repository === body.repository)!.configured =
          false;
      if (request.method() === "POST" && denyCheck) {
        await route.fulfill({
          status: 400,
          json: {
            success: false,
            message: "GitHub returned HTTP 403. Check token permissions.",
          },
        });
        return;
      }
      await route.fulfill({
        json: {
          success: true,
          data: request.method() === "GET" ? entries : null,
        },
      });
    },
  );
  await page.goto("/?mode=github-credentials");
  const input = page.getByPlaceholder("github_pat_…");
  await expect(input).toHaveAttribute("type", "password");
  await input.fill("github_pat_test_initial");
  await page.getByRole("button", { name: "Save token", exact: true }).click();
  await expect(input).toHaveValue("");
  await expect(
    page.getByText("Token registered (never displayed)."),
  ).toBeVisible();
  expect(saves[0]).toEqual({
    repository: "owner/first",
    token: "github_pat_test_initial",
  });
  await page.getByRole("button", { name: "Check connection" }).click();
  await expect(
    page.getByText(/write permissions have not been tested/),
  ).toBeVisible();
  denyCheck = true;
  await page.getByRole("button", { name: "Check connection" }).click();
  await expect(page.getByRole("alert")).toContainText("403");
  await input.fill("github_pat_test_replacement");
  await page.getByRole("button", { name: "Replace token" }).click();
  await expect(input).toHaveValue("");
  expect(saves[1].token).toBe("github_pat_test_replacement");
  await input.fill("github_pat_unsaved");
  await page.getByRole("combobox").selectOption("owner/second");
  await expect(input).toHaveValue("");
  await expect(
    page.getByRole("button", { name: "Check connection" }),
  ).toBeDisabled();
  await page.getByRole("combobox").selectOption("owner/first");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Remove token" }).click();
  await expect(page.getByText(/No LVK token registered/)).toBeVisible();
  await expect(page.locator("body")).not.toContainText(
    "github_pat_test_initial",
  );
  await expect(page.locator("body")).not.toContainText(
    "github_pat_test_replacement",
  );
});
