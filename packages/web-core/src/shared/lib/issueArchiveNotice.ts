import type { TFunction } from 'i18next';

export interface IssueArchiveReceipt {
  request_id: string;
  status: string;
  result: string | null;
}

// Only locally created instances expose archive-specific recovery guidance.
export class IssueArchiveActionError extends Error {
  readonly notice:
    | { kind: 'request'; requestId: string }
    | { kind: 'receipt'; receipt: IssueArchiveReceipt };
  constructor(
    notice:
      | { kind: 'request'; requestId: string }
      | { kind: 'receipt'; receipt: IssueArchiveReceipt }
  ) {
    super('issueArchiveUnconfirmed');
    this.name = 'IssueArchiveActionError';
    this.notice =
      notice.kind === 'receipt'
        ? { kind: 'receipt', receipt: { ...notice.receipt } }
        : { ...notice };
  }
}

export function receiptMessage(
  receipt: IssueArchiveReceipt,
  t: TFunction<'tasks'>
): string {
  if (receipt.status !== 'succeeded')
    return t('tasks:issueArchive.receiptUnconfirmed', {
      requestId: receipt.request_id,
    });
  let result: unknown;
  try {
    result = receipt.result ? JSON.parse(receipt.result) : {};
  } catch {
    return t('tasks:issueArchive.receiptUnreadable', {
      requestId: receipt.request_id,
    });
  }
  if (!result || typeof result !== 'object' || Array.isArray(result))
    return t('tasks:issueArchive.receiptUnreadable', {
      requestId: receipt.request_id,
    });
  const workspaces = (result as { workspaces?: unknown }).workspaces;
  if (workspaces === undefined) return '';
  if (
    !Array.isArray(workspaces) ||
    workspaces.some((ws) => !ws || typeof ws !== 'object' || Array.isArray(ws))
  )
    return t('tasks:issueArchive.receiptUnreadable', {
      requestId: receipt.request_id,
    });
  const warnings = workspaces.filter((ws) => ws.deletion_error);
  if (!warnings.length) return '';
  const workspaceIds = warnings
    .map((ws) =>
      typeof ws.workspace_id === 'string'
        ? ws.workspace_id
        : t('tasks:issueArchive.unknownWorkspace')
    )
    .join('\n');
  return t('tasks:issueArchive.deletionWarnings', {
    requestId: receipt.request_id,
    workspaceIds,
  });
}
