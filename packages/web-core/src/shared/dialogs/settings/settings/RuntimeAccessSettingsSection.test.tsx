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
          (v, p) =>
            v && typeof v === 'object'
              ? (v as Record<string, unknown>)[p]
              : undefined,
          ja
        ) ?? key,
  }),
}));
describe('runtime settings UI', () => {
  it('shows independent permissions, multiline roots and the operational boundary', () => {
    const client = new QueryClient();
    client.setQueryData(['runtime-access-settings'], {
      revision: 1,
      source: 'saved',
      settings: {
        files_enabled: true,
        execution_enabled: false,
        file_roots: ['/srv/logs', '/srv/data'],
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
    expect(html).toContain('MCP経由のファイル読み書きを許可');
    expect(html).toContain('MCP経由のプログラム実行を許可');
    expect(html).toContain('これはサンドボックスではありません');
    expect(html).toContain('/srv/logs\n/srv/data');
    expect(html).not.toContain('許可実行ファイル');
    expect(html).not.toContain('領域を追加');
  });
});
