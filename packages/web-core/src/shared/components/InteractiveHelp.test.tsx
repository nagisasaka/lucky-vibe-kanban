import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { InteractiveHelp } from './InteractiveHelp';

vi.mock('@/shared/lib/helpApi', () => ({
  helpApi: { list: vi.fn(), get: vi.fn() },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/shared/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' }),
  getResolvedTheme: () => 'light',
}));
vi.mock('./MarkdownPreview', () => ({
  MarkdownPreview: ({ content }: { content: string }) =>
    createElement('p', null, content),
}));

function render(status: string, error?: string, oldSource = false) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  client.setQueryData(
    ['help-conversations'],
    [{ id: 'help-session', name: 'Help' }]
  );
  client.setQueryData(['help-conversation', 'help-session'], {
    workspace_id: 'help-workspace',
    session_id: 'help-session',
    source: { build_revision: 'release-sha', commit: 'release-sha' },
    questions: [
      {
        agent_run_id: 'run',
        prompt: 'How do I send a PR?',
        ...(oldSource
          ? { source_commit: 'old-sha', build_revision: 'old-build' }
          : {}),
      },
    ],
    runs: [
      {
        agent_run_id: 'run',
        state: {
          status,
          last_error: error ? { message: error } : null,
          terminal_output:
            status === 'succeeded'
              ? {
                  content: `See CONTRIBUTING.md at ${oldSource ? 'old-sha' : 'release-sha'}.`,
                }
              : null,
        },
      },
    ],
  });
  const markup = renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client },
      createElement(InteractiveHelp, { initialSessionId: 'help-session' })
    )
  );
  client.clear();
  return markup;
}

describe('interactive help conversation', () => {
  it('shows source revision, question, persisted answer and continuation/deletion', () => {
    const markup = render('succeeded');
    expect(markup).toContain('release-sha');
    expect(markup).toContain('How do I send a PR?');
    expect(markup).toContain('See CONTRIBUTING.md');
    expect(markup).toContain('interactiveHelp.status.succeeded');
    expect(markup).toContain('interactiveHelp.delete');
    expect(markup).not.toContain('interactiveHelp.cancel<');
  });
  it('keeps historical answer revision distinct from the current distribution', () => {
    const markup = render('succeeded', undefined, true);
    expect(markup).toContain('interactiveHelp.currentSource: release-sha');
    const article = markup.slice(
      markup.indexOf('<article'),
      markup.indexOf('</article>')
    );
    expect(article).toContain(
      'interactiveHelp.answerSource: old-build · old-sha'
    );
    expect(article).not.toContain('interactiveHelp.answerSource: release-sha');
  });
  it('does not assume the current revision when an old question lacks metadata', () => {
    const markup = render('succeeded');
    expect(markup).toContain(
      'interactiveHelp.answerSource: interactiveHelp.unknownRevision'
    );
  });
  it.each([
    'pending',
    'starting',
    'running',
    'awaiting_input',
    'awaiting_approval',
    'cancelling',
  ])('keeps %s distinct from an answer and offers cancellation', (status) => {
    const markup = render(status);
    expect(markup).toContain(`interactiveHelp.status.${status}`);
    expect(markup).toContain('interactiveHelp.cancel');
    expect(markup).not.toContain('interactiveHelp.delete');
    expect(markup).not.toContain('interactiveHelp.status.succeeded');
    expect(markup).toContain('href="/workspaces/help-workspace"');
  });
  it.each(['failed', 'cancelled', 'crashed', 'audit_failed'])(
    'does not present %s as success',
    (status) => {
      const markup = render(status, 'provider unavailable');
      expect(markup).toContain(`interactiveHelp.status.${status}`);
      expect(markup).toContain('role="alert"');
      expect(markup).toContain('provider unavailable');
      expect(markup).not.toContain('interactiveHelp.status.succeeded');
    }
  );
});
