import { makeLocalApiRequest } from './localApiTransport';

export interface IssueDeleteTarget {
  workspace_id: string;
  name: string | null;
  branch: string;
  path: string | null;
  excluded: boolean;
  reason: string | null;
}
export interface PreparedIssueDelete {
  confirmation_token: string;
  workspaces: IssueDeleteTarget[];
}
export interface IssueDeleteReceipt {
  request_id: string;
  status: string;
  result: string | null;
}
export interface IssueDeleteResult {
  issue_deleted: true;
  workspaces: {
    workspace_id: string;
    db_deleted: true;
    cleanup_completed: boolean;
    cleanup_errors: string[];
  }[];
}
// A successful receipt alone is insufficient: require every confirmed target's result.
export function completedIssueDelete(
  receipt: IssueDeleteReceipt,
  expected: IssueDeleteTarget[],
  requestId: string
): IssueDeleteResult | null {
  if (
    receipt.request_id !== requestId ||
    receipt.status !== 'succeeded' ||
    !receipt.result
  )
    return null;
  try {
    const result = JSON.parse(receipt.result);
    const ids = expected
      .filter((ws) => !ws.excluded)
      .map((ws) => ws.workspace_id);
    if (
      result?.issue_deleted !== true ||
      !Array.isArray(result.workspaces) ||
      result.workspaces.length !== ids.length ||
      new Set(
        result.workspaces.map(
          (ws: { workspace_id: string }) => ws?.workspace_id
        )
      ).size !== ids.length ||
      result.workspaces.some(
        (ws: IssueDeleteResult['workspaces'][number]) =>
          !ws ||
          !ids.includes(ws.workspace_id) ||
          ws.db_deleted !== true ||
          typeof ws.cleanup_completed !== 'boolean' ||
          !Array.isArray(ws.cleanup_errors) ||
          ws.cleanup_errors.some((error) => typeof error !== 'string') ||
          ws.cleanup_completed !== (ws.cleanup_errors.length === 0)
      )
    )
      return null;
    return result;
  } catch {
    return null;
  }
}
async function request<T>(path: string, body?: unknown): Promise<T> {
  const response = await makeLocalApiRequest(
    `/api/board/${path}`,
    body === undefined
      ? {}
      : {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        }
  );
  const value = await response.json();
  if (!response.ok || value.success === false)
    throw new Error(value.message || `HTTP ${response.status}`);
  return value.data;
}
export const issueDeleteApi = {
  prepare: (project_id: string, issue: string) =>
    request<PreparedIssueDelete>(`issues/${issue}/delete/prepare`, {
      project_id,
    }),
  confirm: (
    project_id: string,
    issue: string,
    confirmation_token: string,
    request_id: string
  ) =>
    request<IssueDeleteReceipt>(`issues/${issue}/delete/confirm`, {
      project_id,
      confirmation_token,
      request_id,
    }),
  receipt: (requestId: string) =>
    request<IssueDeleteReceipt>(`issue-delete-operations/${requestId}`),
};
