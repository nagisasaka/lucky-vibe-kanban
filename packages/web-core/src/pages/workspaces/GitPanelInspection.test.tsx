import i18n from '@/i18n/config';
import {
  mapGhCliErrorToUi,
  WorkspaceFromPrFailure,
  workspaceFromPrNotice,
} from '@/shared/lib/githubCliNotice';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { GitPanel } from '@vibe/ui/components/GitPanel';

describe('Git inspection', () => {
  function render(readOnly: boolean) {
    return renderToStaticMarkup(
      createElement(GitPanel, {
        readOnly,
        repos: [
          {
            id: 'repo',
            name: 'Repository',
            targetBranch: 'test/target',
            commitsAhead: 1,
            commitsBehind: 0,
            showPushButton: true,
          },
        ],
        workingBranchName: 'execution/source',
        onWorkingBranchNameChange: vi.fn(),
      })
    );
  }
  it('retains branch metadata without editing, PR, target, or push controls', () => {
    const html = render(true);
    expect(html).toContain('Repository');
    expect(html).toContain('test/target');
    expect(html).toContain('execution/source');
    expect(html).not.toContain('<button');
    expect(html).not.toContain('<input');
    expect(html).not.toContain('Open pull request');
  });
  it('preserves ordinary Git controls', () => {
    expect(render(false)).toContain('Open pull request');
    expect(render(false)).toContain('<button');
  });
});

it('keeps CLI recovery categories and hides arbitrary setup bodies in both locales', () => {
  for (const language of ['en', 'ja']) {
    const t = i18n.getFixedT(language, 'tasks');
    expect(mapGhCliErrorToUi('BREW_MISSING', t).variant).toBe('homebrew');
    expect(mapGhCliErrorToUi('SETUP_HELPER_NOT_SUPPORTED', t).variant).toBe(
      'manual'
    );
    const unknown = mapGhCliErrorToUi(
      { OTHER: { message: 'Raw setup secret /internal/path' } },
      t
    );
    expect(unknown).toEqual(mapGhCliErrorToUi(null, t));
    expect(unknown.message).not.toContain('Raw setup secret');
    expect(unknown.message).not.toContain('githubCliNotice.');
    expect(unknown.message).toContain(
      language === 'ja' ? '確認できません' : 'could not be confirmed'
    );
    expect(unknown.variant).toBeNull();
  }
});

it('formats semantic PR workspace failures in the current locale without exposing provider bodies', () => {
  const failure = new WorkspaceFromPrFailure({
    type: 'auth_failed',
    message: 'Raw auth secret /internal/path',
  });
  for (const language of ['en', 'ja']) {
    const t = i18n.getFixedT(language, 'tasks');
    expect(workspaceFromPrNotice(failure.reason, 'create', t)).toContain(
      language === 'ja'
        ? '認証を確認できません'
        : 'authentication could not be confirmed'
    );
    expect(
      workspaceFromPrNotice(
        { type: 'branch_fetch_failed', message: 'Raw branch secret' },
        'create',
        t
      )
    ).not.toContain('Raw branch secret');
    expect(
      workspaceFromPrNotice(
        { type: 'cli_not_installed', provider: 'git_hub' },
        'load',
        t
      )
    ).toContain('GitHub CLI');
    expect(workspaceFromPrNotice(undefined, 'create', t)).toContain(
      language === 'ja'
        ? '作成結果を確認できません'
        : 'creation could not be confirmed'
    );
    expect(workspaceFromPrNotice(undefined, 'load', t)).not.toContain(
      'createWorkspaceFromPr.'
    );
  }
  expect(failure.message).toBe('auth_failed');
  expect(new Error('Raw auth secret') instanceof WorkspaceFromPrFailure).toBe(
    false
  );
});
