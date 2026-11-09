import i18n from '@/i18n/config';
import { beforeEach } from 'vitest';
beforeEach(async () => {
  await i18n.changeLanguage('en');
});
import { createElement } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WorkspaceWikiPanel } from './WorkspaceWikiPanel';
import type { Workspace } from 'shared/types';

vi.mock('@/shared/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' }),
  getResolvedTheme: () => 'light',
}));
vi.mock('@/shared/components/MarkdownPreview', () => ({
  MarkdownPreview: () => null,
}));

afterEach(() => vi.unstubAllGlobals());

describe('OpenWiki-only Viewer', () => {
  const workspace = {
    id: 'workspace-one',
    workspace_kind: 'direct_folder',
    name: 'Current repo',
  } as Workspace;
  it('always displays read-only OpenWiki for the current workspace', () => {
    const html = renderToStaticMarkup(
      createElement(
        QueryClientProvider,
        { client: new QueryClient() },
        createElement(WorkspaceWikiPanel, { workspace, repos: [] })
      )
    );
    expect(html).toContain('openwiki/');
    expect(html).toContain('Current workspace');
  });
});
