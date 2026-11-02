import { IssueArchiveActionError, receiptMessage } from './issueArchiveNotice';
import i18n from '@/i18n/config';

export type ActionErrorCode =
  | 'executionOnly'
  | 'workspaceRequired'
  | 'repositoryRequired'
  | 'issuesRequired'
  | 'forcePushRequired'
  | 'copyPath'
  | 'noSetupScript'
  | 'noCleanupScript'
  | 'noArchiveScript'
  | 'processRunning'
  | 'singleDuplicate';

// Only locally classified errors expose specific guidance; arbitrary server bodies stay private.
export class ActionError extends Error {
  constructor(readonly code: ActionErrorCode) {
    super(code);
    this.name = 'ActionError';
  }
}

export function actionErrorMessage(error: unknown): string {
  if (error instanceof IssueArchiveActionError) {
    if (error.notice.kind === 'request')
      return i18n.t('tasks:issueArchive.restoreUnconfirmed', {
        requestId: error.notice.requestId,
      });
    return (
      receiptMessage(error.notice.receipt, i18n.getFixedT(null, 'tasks')) ||
      i18n.t('tasks:actionErrors.unconfirmed')
    );
  }
  if (error instanceof ActionError) {
    switch (error.code) {
      case 'executionOnly':
        return i18n.t('tasks:actionErrors.executionOnly');
      case 'workspaceRequired':
        return i18n.t('tasks:actionErrors.workspaceRequired');
      case 'repositoryRequired':
        return i18n.t('tasks:actionErrors.repositoryRequired');
      case 'issuesRequired':
        return i18n.t('tasks:actionErrors.issuesRequired');
      case 'forcePushRequired':
        return i18n.t('tasks:actionErrors.forcePushRequired');
      case 'copyPath':
        return i18n.t('tasks:actionErrors.copyPath');
      case 'noSetupScript':
        return i18n.t('tasks:actionErrors.noSetupScript');
      case 'noCleanupScript':
        return i18n.t('tasks:actionErrors.noCleanupScript');
      case 'noArchiveScript':
        return i18n.t('tasks:actionErrors.noArchiveScript');
      case 'processRunning':
        return i18n.t('tasks:actionErrors.processRunning');
      case 'singleDuplicate':
        return i18n.t('tasks:actionErrors.singleDuplicate');
    }
  }
  return i18n.t('tasks:actionErrors.unconfirmed');
}
