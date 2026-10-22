import { expect, test } from "@playwright/test";
import { seedIssueComments } from "../fixtures/issue-comments.mjs";

// UI/API E2E only. Authenticated MCP paging/scopes are exercised by Rust tests.
test("shared review comments retain attribution, show older targets and allow ordinary long comments", async ({
  page,
  request,
}) => {
  test.skip(
    process.env.LVK_E2E_ISSUE_COMMENTS !== "1",
    "Use the isolated issue-comments runner",
  );
  const fixture = seedIssueComments(process.env.LVK_INSTANCE_DIR!);
  const access = await (await request.get("/api/access/status")).json();
  const api = async (
    path: string,
    data?: unknown,
    method = data === undefined ? "GET" : "POST",
  ) => {
    const response = await request.fetch(path, {
      method,
      data,
      headers: { "x-lvk-csrf": access.csrf },
    });
    expect(response.ok(), await response.text()).toBeTruthy();
    const body = await response.json();
    if ("success" in body) expect(body.success).toBe(true);
    return body.data ?? body;
  };
  const info = await api("/api/info");
  await api(
    "/api/config",
    {
      ...info.config,
      onboarding_acknowledged: true,
      disclaimer_acknowledged: true,
      language: "EN",
      show_release_notes: false,
      showcases: { seen_features: ["workspaces-guide"] },
    },
    "PUT",
  );
  const commentsUrl = `/api/local/v1/fallback/issue_comments?issue_id=${fixture.issue}`;
  const issueUrl = `/api/local/v1/issues/${fixture.issue}`;
  const longBody = "界😀e\u0301".repeat(5000) + "\nLONG COMMENT END";
  const ordinary = await api("/api/local/v1/issue_comments", {
    issue_id: fixture.issue,
    message: longBody,
    parent_id: null,
  });
  const records = (await api(commentsUrl)).issue_comments;
  expect(records).toHaveLength(3);
  expect(
    records.find((row: { id: string }) => row.id === ordinary.id).message,
  ).toBe(longBody);
  expect(
    records.every(
      (row: { current_issue_revision: string }) =>
        row.current_issue_revision === fixture.revision,
    ),
  ).toBe(true);
  await page.goto(`/projects/${fixture.project}/issues/${fixture.issue}`);
  const provenance = page.getByTestId("comment-provenance");
  await expect(provenance).toHaveCount(2);
  await expect(provenance.filter({ hasText: "ChatGPT" })).toContainText(
    "self-reported",
  );
  await expect(provenance.filter({ hasText: "Dot" })).toContainText(
    "shared-user",
  );
  await expect(
    page.getByText("LONG COMMENT END", { exact: false }),
  ).toBeVisible();
  await api(issueUrl, { description: "Revised agreed requirements" }, "PATCH");
  await page.reload();
  await expect(provenance.first()).toContainText(
    "Different from current Issue revision",
  );
  const issueRevision = (await api(commentsUrl)).issue_comments[0]
    .current_issue_revision;
  await api(
    `/api/local/v1/issue_comments/${fixture.comments[0]}`,
    { message: "Edited review body" },
    "PATCH",
  );
  await page.reload();
  await expect(provenance.filter({ hasText: "ChatGPT" })).toContainText(
    "Last edited at",
  );
  const edited = (await api(commentsUrl)).issue_comments.find(
    (row: { id: string }) => row.id === fixture.comments[0],
  );
  expect(edited.target_revision).toBe(fixture.revision);
  expect(edited.current_issue_revision).toBe(issueRevision);
  expect(edited.recorded_principal).toBe('["fixture-issuer","shared-user"]');
  expect(edited.message).toBe("Edited review body");
});
