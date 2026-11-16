import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import type { AttachmentResponse } from 'shared/types';
import { IssueAttachmentsSection } from './IssueAttachmentsSection';
import { issueAttachmentsApi } from '@/shared/lib/api';

const { queries } = vi.hoisted(() => ({ queries: [] as unknown[] }));
vi.mock('@tanstack/react-query', async (original) => {
  const actual = await original<typeof import('@tanstack/react-query')>();
  return {
    ...actual,
    useQuery: (options: Parameters<typeof actual.useQuery>[0]) => {
      queries.push(options);
      return actual.useQuery(options);
    },
  };
});
vi.mock('@/shared/lib/api', () => ({
  issueAttachmentsApi: { list: vi.fn(), upload: vi.fn(), download: vi.fn() },
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'ja' } }),
}));

const attachment = (name: string): AttachmentResponse => ({
  id: name,
  original_name: name,
  size_bytes: 1536n,
  file_path: name,
  mime_type: 'text/plain',
  hash: name,
  created_at: '',
  updated_at: '',
});

describe('issue attachment panel', () => {
  it('keeps delayed responses separate when switching Issue and Project', async () => {
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false, retryOnMount: false, staleTime: Infinity },
      },
    });
    let resolveOld!: (value: AttachmentResponse[]) => void;
    vi.mocked(issueAttachmentsApi.list).mockImplementation((project, issue) =>
      issue === 'old'
        ? new Promise((resolve) => {
            resolveOld = resolve;
          })
        : Promise.resolve([attachment(`${project}-${issue}.txt`)])
    );
    const render = (projectId: string, issueId: string) =>
      renderToStaticMarkup(
        createElement(
          QueryClientProvider,
          { client },
          createElement(IssueAttachmentsSection, { projectId, issueId })
        )
      );
    const load = () =>
      client.fetchQuery(
        queries.at(-1) as Parameters<typeof client.fetchQuery>[0]
      );
    try {
      expect(render('project-a', 'old')).toContain('states.loading');
      const delayed = load();
      render('project-a', 'current');
      await load();
      expect(render('project-a', 'current')).toContain('project-a-current.txt');
      resolveOld([attachment('old-private.txt')]);
      await delayed;
      expect(render('project-a', 'current')).not.toContain('old-private.txt');
      expect(render('project-b', 'current')).not.toContain(
        'project-a-current.txt'
      );
      await load();
      expect(render('project-b', 'current')).toContain('project-b-current.txt');
      expect(render('project-b', 'current')).toContain('1.5 KB');
    } finally {
      client.clear();
    }
  });

  it('shows empty and failed lists with a refresh action', async () => {
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false, retryOnMount: false, staleTime: Infinity },
      },
    });
    const render = (issueId: string) =>
      renderToStaticMarkup(
        createElement(
          QueryClientProvider,
          { client },
          createElement(IssueAttachmentsSection, {
            projectId: 'project',
            issueId,
          })
        )
      );
    try {
      client.setQueryData(['issue-attachments', 'project', 'empty'], []);
      expect(render('empty')).toContain('kanban.attachments.empty');
      vi.mocked(issueAttachmentsApi.list).mockRejectedValue(
        new Error('denied')
      );
      render('denied');
      await expect(
        client.fetchQuery(
          queries.at(-1) as Parameters<typeof client.fetchQuery>[0]
        )
      ).rejects.toThrow('denied');
      const markup = render('denied');
      expect(markup).toContain('role="alert"');
      expect(markup).toContain('kanban.attachments.loadFailed');
      expect(markup).toContain('kanban.attachments.retry');
    } finally {
      client.clear();
    }
  });
});
