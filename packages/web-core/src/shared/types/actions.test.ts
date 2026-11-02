import { describe, expect, it } from 'vitest';
import i18n from '@/i18n/config';
import { Actions } from '@/shared/actions';
import type { Workspace } from 'shared/types';
import {
  ActionTargetType,
  getActionLabel,
  getActionTooltip,
  isActionEnabled,
  isActionVisible,
  type ActionDefinition,
  type ActionVisibilityContext,
} from './actions';

const context = {
  workspaceReadOnly: true,
  workspaceArchived: false,
} as ActionVisibilityContext;
function action(id: string, requiresTarget = ActionTargetType.WORKSPACE) {
  return {
    id,
    requiresTarget,
    isVisible: () => true,
    isEnabled: () => true,
  } as ActionDefinition;
}

describe('execution-only action policy', () => {
  it('allows manual archive but keeps unarchive restricted', () => {
    const archive = action('archive-workspace');
    expect(isActionVisible(archive, context)).toBe(true);
    expect(isActionEnabled(archive, context)).toBe(true);
    const archived = { ...context, workspaceArchived: true };
    expect(isActionVisible(archive, archived)).toBe(false);
    expect(isActionEnabled(archive, archived)).toBe(false);
  });
  it.each([
    'duplicate-workspace',
    'rename-workspace',
    'delete-workspace',
    'start-review',
    'spin-off-workspace',
    'run-setup-script',
    'run-cleanup-script',
    'run-archive-script',
    'future-write-operation',
  ])('blocks %s independently of the owner feature', (id) => {
    expect(isActionVisible(action(id), context)).toBe(false);
    expect(isActionEnabled(action(id), context)).toBe(false);
    expect(
      isActionEnabled(action(id), { ...context, workspaceReadOnly: false })
    ).toBe(true);
  });
  it.each([
    'git-merge',
    'git-rebase',
    'git-push',
    'git-create-pr',
    'git-change-target',
    'repo-open-in-ide',
  ])('blocks Git mutation %s', (id) =>
    expect(isActionEnabled(action(id, ActionTargetType.GIT), context)).toBe(
      false
    )
  );
  it.each(['open-in-ide', 'toggle-dev-server', 'toggle-preview-mode'])(
    'also blocks global workspace side effects: %s',
    (id) =>
      expect(isActionVisible(action(id, ActionTargetType.NONE), context)).toBe(
        false
      )
  );
  it.each([
    'toggle-files-mode',
    'toggle-wiki-mode',
    'toggle-logs-mode',
    'toggle-changes-mode',
    'copy-workspace-path',
    'new-workspace',
  ])('preserves inspection/navigation: %s', (id) =>
    expect(isActionEnabled(action(id, ActionTargetType.NONE), context)).toBe(
      true
    )
  );
  it('keeps repository path copy available', () =>
    expect(
      isActionEnabled(action('repo-copy-path', ActionTargetType.GIT), context)
    ).toBe(true));
});

// Catalog resolution is independent of DOM layout; browser tests retain search and mutation boundaries.
describe('localized action catalog', () => {
  it('resolves all labels/tooltips without changing identity or policy', () => {
    const ctx = {
      ...context,
      layoutMode: 'workspaces',
      isLeftMainPanelVisible: false,
      selectedIssueArchived: false,
      devServerState: 'stopped',
      logsPanelContent: null,
      hasWorkspace: true,
      isCreatingIssue: false,
    } as ActionVisibilityContext;
    const workspace = {
      id: 'raw-workspace',
      pinned: false,
      archived: false,
    } as Workspace;
    const identity = Object.values(Actions).map((action) => [
      action.id,
      isActionEnabled(action as ActionDefinition, ctx),
    ]);
    for (const language of ['en', 'ja']) {
      const t = i18n.getFixedT(language, 'tasks');
      for (const action of Object.values(Actions)) {
        for (const label of [
          getActionLabel(action as ActionDefinition, ctx, workspace, t),
          getActionTooltip(action as ActionDefinition, ctx, t),
        ]) {
          expect(label).not.toBe('');
          expect(label).not.toContain('tasks:');
        }
      }
      expect(
        Object.values(Actions).map((action) => [
          action.id,
          isActionEnabled(action as ActionDefinition, ctx),
        ])
      ).toEqual(identity);
      expect(workspace).toEqual({
        id: 'raw-workspace',
        pinned: false,
        archived: false,
      });
    }
  });

  it.each([
    [
      'en',
      [
        'Pin',
        'Unpin',
        'Archive',
        'Unarchive',
        'Show Chat Panel',
        'Hide Chat Panel',
        'Link Workspace (available after unarchiving)',
      ],
    ],
    [
      'ja',
      [
        'ピン留め',
        'ピン留めを解除',
        'アーカイブ',
        'アーカイブを解除',
        'チャットパネルを表示',
        'チャットパネルを隠す',
        'ワークスペースをリンク（アーカイブを解除すると利用できます）',
      ],
    ],
  ] as const)(
    'resolves %s state-dependent labels without DOM fixtures',
    (language, labels) => {
      const t = i18n.getFixedT(language, 'tasks');
      const initial = {
        ...context,
        selectedIssueArchived: false,
        isLeftMainPanelVisible: false,
      } as ActionVisibilityContext;
      const changed = {
        ...initial,
        selectedIssueArchived: true,
        isLeftMainPanelVisible: true,
      };
      const workspace = { pinned: false } as Workspace;
      expect([
        getActionLabel(Actions.PinWorkspace, initial, workspace, t),
        getActionLabel(
          Actions.PinWorkspace,
          changed,
          { ...workspace, pinned: true },
          t
        ),
        getActionLabel(Actions.ArchiveIssue, initial, workspace, t),
        getActionLabel(Actions.ArchiveIssue, changed, workspace, t),
        getActionLabel(Actions.ToggleLeftMainPanel, initial, workspace, t),
        getActionLabel(Actions.ToggleLeftMainPanel, changed, workspace, t),
        getActionLabel(Actions.LinkWorkspace, changed, workspace, t),
      ]).toEqual(labels);
    }
  );
});
