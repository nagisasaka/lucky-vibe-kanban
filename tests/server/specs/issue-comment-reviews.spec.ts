import { readFileSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import { seedIssueComments } from "../fixtures/issue-comments.mjs";

// Real JWT-authenticated MCP writes and real UI/API share one disposable DB.
test("shared review comments retain attribution, show older targets and allow ordinary long comments", async ({
  page,
  request,
}) => {
  test.skip(
    process.env.LVK_E2E_ISSUE_COMMENTS !== "1",
    "Use the isolated issue-comments runner",
  );
  const fixture = seedIssueComments(process.env.LVK_INSTANCE_DIR!, {
    reviews: false,
  });
  const credentials = JSON.parse(
    readFileSync(process.env.LVK_INSTANCE_DIR! + "/mcp-fixture.json", "utf8"),
  );
  const mcp = async (name: string, args: unknown, reader = false) => {
    const response = await request.post(credentials.endpoint, {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        Host: credentials.host,
        "MCP-Protocol-Version": "2025-03-26",
        Authorization: `Bearer ${reader ? credentials.reader : credentials.writer}`,
      },
      data: {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: { name, arguments: args },
      },
    });
    const responseBody = await response.text();
    expect(response.status(), responseBody).toBe(200);
    const result = JSON.parse(responseBody);
    expect(result.result.isError).not.toBe(true);
    return result.result.structuredContent;
  };
  const target = { project_id: fixture.project, issue_id: fixture.issue };
  const issue = await mcp("get_issue", target, true);
  const inputs = ["ChatGPT", "Dot"].map((label) => ({
    ...target,
    request_id: randomUUID(),
    message: `## ${label} review\nIndependent opinion from MCP`,
    reviewer_label: label,
    review_target: issue.review_target,
  }));
  const receipts = await Promise.all(
    inputs.map((input) => mcp("add_issue_comment", input)),
  );
  for (const receipt of receipts) expect(receipt.status).toBe("succeeded");
  fixture.comments = receipts.map((receipt) => receipt.result.comment_id);
  expect((await mcp("add_issue_comment", inputs[0])).result.comment_id).toBe(
    fixture.comments[0],
  );
  expect((await mcp("list_issue_comments", target, true)).items).toHaveLength(
    2,
  );
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
  const listing = await mcp(
    "list_issue_comments",
    { ...target, limit: 50 },
    true,
  );
  expect(
    listing.items.find((item: { id: string }) => item.id === ordinary.id)
      .message_truncated,
  ).toBe(true);
  let full = "",
    offset = 0,
    revision: string | undefined;
  do {
    const part = await mcp(
      "get_issue_comment",
      {
        ...target,
        comment_id: ordinary.id,
        offset,
        limit: 997,
        expected_comment_revision: revision,
      },
      true,
    );
    full += part.message_page;
    revision = part.comment_revision;
    if (!part.has_more) break;
    offset = part.next_offset;
  } while (true);
  expect(full).toBe(longBody);
  await page.goto(`/projects/${fixture.project}/issues/${fixture.issue}`);
  const provenance = page.getByTestId("comment-provenance");
  await expect(provenance).toHaveCount(2);
  await expect(provenance.filter({ hasText: "ChatGPT" })).toContainText(
    "self-reported",
  );
  await expect(provenance.filter({ hasText: "Dot" })).toContainText("member");
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
  expect(edited.recorded_principal).toBe(
    '["https://fixture.auth0.com/","member"]',
  );
  expect(edited.message).toBe("Edited review body");
  await expect(
    page.getByText("Edited review body", { exact: true }),
  ).toBeVisible();
  await page.setViewportSize({ width: 1440, height: 1000 });
  await provenance.filter({ hasText: "ChatGPT" }).scrollIntoViewIfNeeded();
  await page.screenshot({
    path: ".build/local88/issue-comments-final.png",
    fullPage: false,
  });
});
