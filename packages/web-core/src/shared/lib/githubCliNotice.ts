import type {
  CreateFromPrError,
  GhCliSetupError,
  ListPrsError,
} from 'shared/types';
import type { TFunction } from 'i18next';

export type GhCliSupportVariant = 'homebrew' | 'manual';
export interface GhCliSupportContent {
  message: string;
  variant: GhCliSupportVariant | null;
}

export function mapGhCliErrorToUi(
  error: GhCliSetupError | null,
  t: (key: string) => string
): GhCliSupportContent {
  switch (error) {
    case 'BREW_MISSING':
      return {
        message: t('settings:integrations.github.cliSetup.errors.brewMissing'),
        variant: 'homebrew',
      };
    case 'SETUP_HELPER_NOT_SUPPORTED':
      return {
        message: t('settings:integrations.github.cliSetup.errors.notSupported'),
        variant: 'manual',
      };
    default:
      return {
        message: t('tasks:githubCliNotice.setupUnconfirmed'),
        variant: null,
      };
  }
}

// Trusted local failures keep semantic reasons; arbitrary transport errors stay generic.
export class WorkspaceFromPrFailure extends Error {
  constructor(readonly reason: CreateFromPrError | { type: 'missing_fields' }) {
    super(reason.type);
  }
}

export function workspaceFromPrNotice(
  reason:
    | CreateFromPrError
    | ListPrsError
    | { type: 'missing_fields' }
    | undefined,
  phase: 'load' | 'create',
  t: TFunction<'tasks'>
): string {
  switch (reason?.type) {
    case 'cli_not_installed':
      return t('tasks:createWorkspaceFromPr.errors.cliNotInstalled', {
        provider:
          reason.provider === 'git_hub'
            ? 'GitHub'
            : reason.provider === 'azure_dev_ops'
              ? 'Azure DevOps'
              : 'Git',
      });
    case 'auth_failed':
      return t('tasks:createWorkspaceFromPr.errors.authUnconfirmed');
    case 'branch_fetch_failed':
      return t('tasks:createWorkspaceFromPr.errors.branchFetchUnconfirmed');
    case 'pr_not_found':
      return t('tasks:createWorkspaceFromPr.errors.prNotFound');
    case 'unsupported_provider':
      return t('tasks:createWorkspaceFromPr.errors.unsupportedProvider');
    case 'missing_fields':
      return t('tasks:createWorkspaceFromPr.errors.missingFields');
    default:
      return phase === 'load'
        ? t('tasks:createWorkspaceFromPr.errors.failedToLoadPrs')
        : t('tasks:createWorkspaceFromPr.errors.failedToCreateWorkspace');
  }
}
