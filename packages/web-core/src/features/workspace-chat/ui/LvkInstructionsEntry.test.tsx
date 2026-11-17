import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it, vi } from 'vitest';
import { LvkInstructionsEntry } from './LvkInstructionsEntry';

vi.mock('react-i18next', async () => {
  const { default: messages } = await import('@/i18n/locales/ja/common.json');
  return {
    useTranslation: () => ({
      t: (key: string) =>
        key
          .split('.')
          .reduce(
            (value: unknown, part) => (value as Record<string, unknown>)[part],
            messages
          ),
    }),
  };
});

it('starts folded and shows the saved content, role, route, identity and acknowledgment limits', () => {
  const html = renderToStaticMarkup(
    createElement(LvkInstructionsEntry, {
      runId: 'run-2',
      delivery: {
        id: 'd',
        route: 'thread/inject_items',
        role: 'developer',
        reason: 'current_values',
        nativeSessionId: 'native-1',
        content: '<固定文>\nRUN=2 draft_path=2.json',
        status: 'accepted',
        attemptedAt: 'start',
        updatedAt: 'ack',
      },
    })
  );
  expect(html).toContain('LVK の追加指示');
  expect(html).not.toMatch(/<details[^>]*\bopen\b/);
  expect(html).toContain('&lt;固定文&gt;\nRUN=2 draft_path=2.json');
  expect(html).toContain('thread/inject_items');
  expect(html).toContain('developer');
  expect(html).toContain('run-2 / native-1');
  expect(html).toContain('モデル到達・遵守の証明ではありません');
});

it('distinguishes missing historical evidence and unconfirmed delivery from acknowledgment', () => {
  const missing = renderToStaticMarkup(createElement(LvkInstructionsEntry, {}));
  expect(missing).toContain('配信記録なし');
  expect(missing).toContain('未配信を意味しません');
  expect(missing).not.toContain('<pre');
  const unknown = renderToStaticMarkup(
    createElement(LvkInstructionsEntry, {
      delivery: {
        id: 'd',
        route: 'stream-json.user_suffix',
        role: 'user',
        reason: 'full_fallback',
        nativeSessionId: null,
        content: '保存済み',
        status: 'unconfirmed',
        attemptedAt: 'start',
        updatedAt: 'end',
      },
    })
  );
  expect(unknown).toContain('受付結果不明');
  expect(unknown).not.toContain('· CLI 受付確認');
});
