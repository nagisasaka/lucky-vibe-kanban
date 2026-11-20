import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, beforeAll } from 'vitest';
import {
  IssueWorkspaceCard,
  type WorkspaceWithStats,
} from '@vibe/ui/components/IssueWorkspaceCard';
import { WorkspaceSummary } from '@vibe/ui/components/WorkspaceSummary';
import i18n from '@/i18n/config';

const workspace: WorkspaceWithStats = {
  id: 'workspace',
  localWorkspaceId: 'workspace',
  name: 'Test',
  archived: false,
  prs: [],
  owner: null,
  updatedAt: '2026-01-01',
  isOwnedByCurrentUser: true,
  isRunning: true,
  hasPendingApproval: false,
  hasPendingInput: false,
  hasBlockedGoal: false,
};

describe('workspace attention indicator', () => {
  beforeAll(async () => {
    await i18n.changeLanguage('ja');
  });
  it('shows running normally, then the blocked Goal hand', () => {
    const normal = renderToStaticMarkup(
      createElement(IssueWorkspaceCard, { workspace })
    );
    expect(normal).toContain('animate-running-dot');
    const blocked = renderToStaticMarkup(
      createElement(IssueWorkspaceCard, {
        workspace: { ...workspace, hasBlockedGoal: true },
      })
    );
    expect(blocked).toContain('aria-label="ブロック中"');
    expect(blocked).not.toContain('animate-running-dot');
    const sidebar = renderToStaticMarkup(
      createElement(WorkspaceSummary, {
        name: 'Test',
        isRunning: true,
        hasBlockedGoal: true,
      })
    );
    expect(sidebar).toContain('title="ブロック中"');
    expect(sidebar).not.toContain('animate-running-dot');
  });
  it('keeps approval and input ahead of Goal status', () => {
    const approval = renderToStaticMarkup(
      createElement(IssueWorkspaceCard, {
        workspace: {
          ...workspace,
          hasBlockedGoal: true,
          hasPendingInput: true,
          hasPendingApproval: true,
        },
      })
    );
    expect(approval).toContain('aria-label="承認待ち"');
    const input = renderToStaticMarkup(
      createElement(IssueWorkspaceCard, {
        workspace: {
          ...workspace,
          hasBlockedGoal: true,
          hasPendingInput: true,
        },
      })
    );
    expect(input).toContain('aria-label="入力待ち"');
  });
});
