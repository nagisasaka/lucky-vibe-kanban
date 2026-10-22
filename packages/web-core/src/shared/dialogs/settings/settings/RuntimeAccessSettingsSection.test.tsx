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
    expect(html).not.toContain('対象利用者subject');
    expect(html).not.toContain('許可実行ファイル');
    expect(html).toContain('許可ディレクトリ（絶対パス）');
  });
  it('shows an explicit disabled-empty recovery action and reason, without an enable form', () => {
    const client = new QueryClient();
    client.setQueryData(['runtime-access-settings'], {
      revision: 0,
      source: 'invalid_legacy',
      legacy_error: {
        kind: 'invalid_grant',
        reason: 'Missing configured root',
      },
      projects: [],
      settings: { enabled: false, grants: [] },
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
    expect(html).toContain('Missing configured root');
    expect(html).toContain('無効・空の設定を保存して復旧');
    expect(html).toContain('自動リセットは行いません');
    expect(html).not.toContain('id="runtime-enabled"');
    expect(html).toContain('自然終了を待ちます');
    expect(html).toContain('daemonは自然終了しない可能性');
  });
});
