import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi } from 'vitest';
import { RuntimeAccessSettingsSection } from './RuntimeAccessSettingsSection';
import { SettingsDirtyProvider } from './SettingsDirtyContext';
import ja from '@/i18n/locales/ja/settings.json';
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) =>
      key
        .split('.')
        .reduce<unknown>(
          (value, part) =>
            value && typeof value === 'object'
              ? (value as Record<string, unknown>)[part]
              : undefined,
          ja
        ) ?? key,
  }),
}));
describe('runtime settings UI', () => {
  it('names the feature and exposes explicit execution authority and revocation limits', () => {
    const client = new QueryClient();
    client.setQueryData(['runtime-access-settings'], {
      revision: 1,
      source: 'saved',
      projects: [{ id: 'project', name: 'Example API' }],
      settings: {
        enabled: true,
        grants: [
          {
            id: 'region',
            display_name: 'API logs',
            project_id: 'project',
            root: '/srv/api',
            subjects: ['member'],
            executables: [],
            allow_uncoordinated_execution: false,
          },
        ],
      },
    });
    const html = renderToStaticMarkup(
      createElement(
        QueryClientProvider,
        { client },
        createElement(
          SettingsDirtyProvider,
          null,
          createElement(RuntimeAccessSettingsSection)
        )
      )
    );
    expect(html).toContain('実行環境へのアクセス');
    expect(html).toContain('OSサンドボックスではありません');
    expect(html).toContain('取り消しても稼働中プロセスは終了しません');
    expect(html).toContain('Example API');
    expect(html).toContain('API logs');
  });
});
