import type { TFunction } from 'i18next';

const labels = {
  settings: (t: TFunction<'common'>) => t('common:shortcuts.actions.settings'),
  'new-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.new-workspace'),
  'duplicate-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.duplicate-workspace'),
  'rename-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.rename-workspace'),
  'pin-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.pin-workspace'),
  'archive-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.archive-workspace'),
  'delete-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.delete-workspace'),
  'toggle-changes-mode': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-changes-mode'),
  'toggle-logs-mode': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-logs-mode'),
  'toggle-preview-mode': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-preview-mode'),
  'toggle-left-sidebar': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-left-sidebar'),
  'toggle-left-main-panel': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-left-main-panel'),
  'git-create-pr': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.git-create-pr'),
  'git-merge': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.git-merge'),
  'git-rebase': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.git-rebase'),
  'git-push': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.git-push'),
  'copy-path': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.copy-path'),
  'copy-raw-logs': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.copy-raw-logs'),
  'toggle-dev-server': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-dev-server'),
  'toggle-wrap-lines': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.toggle-wrap-lines'),
  'run-setup-script': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.run-setup-script'),
  'run-cleanup-script': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.run-cleanup-script'),
  'run-archive-script': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.run-archive-script'),
  'create-issue': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.create-issue'),
  'change-issue-status': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.change-issue-status'),
  'change-issue-priority': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.change-issue-priority'),
  'change-assignees': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.change-assignees'),
  'make-sub-issue-of': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.make-sub-issue-of'),
  'add-sub-issue': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.add-sub-issue'),
  'remove-parent-issue': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.remove-parent-issue'),
  'link-workspace': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.link-workspace'),
  'duplicate-issue': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.duplicate-issue'),
  'delete-issue': (t: TFunction<'common'>) =>
    t('common:shortcuts.actions.delete-issue'),
};
export type SequentialActionId = keyof typeof labels;

export function sequentialShortcutLabel(
  t: TFunction<'common'>,
  actionId: SequentialActionId
): string {
  return labels[actionId](t);
}
