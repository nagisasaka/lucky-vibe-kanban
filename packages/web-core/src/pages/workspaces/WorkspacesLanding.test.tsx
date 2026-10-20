import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { WorkspacesLanding } from './WorkspacesLanding';

vi.mock('./Workspaces', () => ({
  Workspaces: () => createElement('main', null, 'Workspace list'),
}));
vi.mock('@/shared/hooks/useAppNavigation', () => ({
  useAppNavigation: () => {
    throw new Error('通常遷移では作成画面に転送しない');
  },
}));

describe('Workspaceの通常遷移', () => {
  it('新規作成への転送ではなく既存の一覧レイアウトを表示する', () => {
    expect(renderToStaticMarkup(createElement(WorkspacesLanding))).toBe(
      '<main>Workspace list</main>'
    );
  });
});
