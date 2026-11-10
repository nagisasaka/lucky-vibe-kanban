import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it, vi } from 'vitest';
import type { IntegrationRun } from 'shared/types';
import { IntegrationInitialMessage } from './IntegrationInitialMessage';

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

it('shows an LVK summary with original instructions and saved input in collapsed details', () => {
  const run = {
    target_ref: 'refs/heads/main',
    payload: {
      observed_target: 'FIXED_BASE',
      instructions: '採用時の追加指示原文',
      wiki_enabled: true,
      skip_validation: false,
      sources: [
        {
          title: '採用したIssue',
          commit: 'FIXED_SOURCE',
          selection: { workspace_id: 'source-workspace' },
        },
      ],
      worker_dispatch: {
        type: 'create',
        data: {
          request: {
            agent_run_id: 'initial-run',
            input: {
              message_id: 'initial-message',
              content: '保存された旧SkillとIssue本文とManifestの原文',
            },
          },
        },
      },
    },
  } as unknown as IntegrationRun;
  const html = renderToStaticMarkup(
    createElement(IntegrationInitialMessage, { run })
  );
  expect(html).toContain('自動マージ</h3>');
  expect(html).toContain('LVKが構成した依頼');
  expect(html).toContain('FIXED_BASE');
  expect(html).toContain('FIXED_SOURCE');
  expect(html).toContain('refs/heads/main');
  expect(html).toContain('同じ候補で更新');
  expect(html).toContain('採用時の追加指示原文');
  expect(html).toContain('保存された旧SkillとIssue本文とManifestの原文');
  expect(html.match(/<details>/g)).toHaveLength(2);
  expect(html).not.toContain('<details open');
  expect(html).not.toContain('あなた');
});
