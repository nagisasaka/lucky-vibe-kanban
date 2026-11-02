import type { IssueArchiveReceipt } from './issueArchiveNotice';
import { makeLocalApiRequest } from './localApiTransport';
import { refreshShapeFallback } from './electric/collections';
import {
  PROJECT_ISSUES_SHAPE,
  PROJECT_WORKSPACES_SHAPE,
  USER_WORKSPACES_SHAPE,
} from 'shared/remote-types';
export interface IssueArchiveTarget {
  workspace_id: string;
  path: string | null;
  branch: string;
  archived: boolean;
  worktree_deleted: boolean;
}
export interface PreparedIssueArchive {
  confirmation_token: string;
  archive_workspaces: boolean;
  workspaces: IssueArchiveTarget[];
  message: string;
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
    throw new Error(value.message || '操作に失敗しました');
  return value.data;
}
export const issueArchiveApi = {
  prepare: (project_id: string, issue: string, archive_workspaces: boolean) =>
    request<PreparedIssueArchive>(`issues/${issue}/archive/prepare`, {
      project_id,
      archive_workspaces,
    }),
  confirm: (
    project_id: string,
    issue: string,
    confirmation_token: string,
    request_id: string
  ) =>
    request<IssueArchiveReceipt>(`issues/${issue}/archive/confirm`, {
      project_id,
      confirmation_token,
      request_id,
    }),
  restore: (project_id: string, issue: string, request_id: string) =>
    request<IssueArchiveReceipt>(`issues/${issue}/unarchive`, {
      project_id,
      request_id,
    }),
  receipt: (requestId: string) =>
    request<IssueArchiveReceipt>(`issue-archive-operations/${requestId}`),
};
export function refreshIssueArchive(projectId: string) {
  refreshShapeFallback(PROJECT_ISSUES_SHAPE, { project_id: projectId });
  refreshShapeFallback(PROJECT_WORKSPACES_SHAPE, { project_id: projectId });
  refreshShapeFallback(USER_WORKSPACES_SHAPE, {});
}
