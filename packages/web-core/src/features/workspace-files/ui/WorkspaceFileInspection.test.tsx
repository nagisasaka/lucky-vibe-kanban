import i18n from '@/i18n/config';
import { beforeEach } from 'vitest';
beforeEach(async () => {
  await i18n.changeLanguage('en');
});
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import type { WorkspaceFilePreviewTarget } from '../model/types';
import { WorkspaceFilePreviewHeader } from './WorkspaceFilePreviewHeader';
import { WorkspaceFilePreviewPanel } from './WorkspaceFilePreviewPanel';
import { UnsupportedFileViewer } from './viewers/UnsupportedFileViewer';

const state = vi.hoisted(() => ({ usage: undefined as string | undefined }));
vi.mock('@/shared/hooks/useWorkspaceRecord', () => ({
  useWorkspaceRecord: () => ({
    data: state.usage ? { usage: state.usage } : undefined,
  }),
}));
vi.mock('@/shared/hooks/useOpenInEditor', () => ({
  useOpenInEditor: () => vi.fn(),
}));
vi.mock('../model/useWorkspaceFileContent', () => ({
  useWorkspaceFileContent: () => ({
    data: undefined,
    isLoading: false,
    isFetching: false,
    error: new Error('Raw internal preview secret'),
    refresh: vi.fn(),
    refetch: vi.fn(),
  }),
}));
vi.mock('@/shared/providers/HostIdProvider', () => ({ useHostId: () => null }));
vi.mock('@/shared/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' }),
}));

describe('passive file inspection controls', () => {
  const target: WorkspaceFilePreviewTarget = {
    workspaceId: 'workspace',
    repoId: 'repo',
    path: 'file.bin',
    source: 'file-tree',
  };
  it('withholds private preview failure details', () => {
    state.usage = 'execution_only';
    const html = renderToStaticMarkup(
      createElement(WorkspaceFilePreviewPanel, { target })
    );
    expect(html).toContain('Could not load preview');
    expect(html).not.toContain('Raw internal preview secret');
    expect(html).not.toContain('Open in editor');
  });
  it.each([undefined, 'execution_only'])(
    'does not offer an editor for usage %s',
    (usage) => {
      state.usage = usage;
      const header = renderToStaticMarkup(
        createElement(WorkspaceFilePreviewHeader, {
          target,
          onRefresh: vi.fn(),
        })
      );
      const unsupported = renderToStaticMarkup(
        createElement(UnsupportedFileViewer, { target, rawUrl: '/safe-raw' })
      );
      expect(header).toContain('Refresh preview');
      expect(header).not.toContain('Open in editor');
      expect(unsupported).toContain('Open raw');
      expect(unsupported).not.toContain('Open editor');
    }
  );
  it('preserves ordinary editor affordances', () => {
    state.usage = 'interactive';
    expect(
      renderToStaticMarkup(
        createElement(WorkspaceFilePreviewHeader, {
          target,
          onRefresh: vi.fn(),
        })
      )
    ).toContain('Open in editor');
    expect(
      renderToStaticMarkup(createElement(UnsupportedFileViewer, { target }))
    ).toContain('Open editor');
  });
});
