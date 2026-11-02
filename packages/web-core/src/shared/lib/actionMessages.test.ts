import { IssueArchiveActionError } from './issueArchiveNotice';
import { afterEach, describe, expect, it } from 'vitest';
import i18n from '@/i18n/config';
import { ActionError, actionErrorMessage } from './actionMessages';

afterEach(async () => {
  await i18n.changeLanguage('en');
});

describe('action error presentation', () => {
  it('retains classified semantics across language changes', async () => {
    const error = new ActionError('forcePushRequired');
    await i18n.changeLanguage('en');
    expect(actionErrorMessage(error)).toBe(
      'The remote branch has diverged. Review it before deciding whether to force push.'
    );
    await i18n.changeLanguage('ja');
    expect(actionErrorMessage(error)).toBe(
      'リモートブランチが分岐しています。強制プッシュの要否を判断する前に内容を確認してください。'
    );
  });

  it('does not display arbitrary provider bodies or trust lookalike error codes', async () => {
    for (const language of ['en', 'ja']) {
      await i18n.changeLanguage(language);
      const expected =
        language === 'ja'
          ? '操作の結果を確認できませんでした。再試行する前に現在の状態を確認してください。'
          : 'The action could not be confirmed. Check the current state before retrying.';
      for (const error of [
        new Error('secret-provider-body'),
        { code: 'forcePushRequired', message: 'secret-provider-body' },
        null,
      ]) {
        expect(actionErrorMessage(error)).toBe(expected);
      }
    }
  });

  it('selects English singular/plural and Japanese count forms without raw keys', async () => {
    await i18n.changeLanguage('en');
    expect(
      i18n.t('tasks:actionDialogs.behind', { count: 1, formattedCount: '1' })
    ).toContain('1 commit behind');
    expect(
      i18n.t('tasks:actionDialogs.behind', { count: 2, formattedCount: '2' })
    ).toContain('2 commits behind');
    await i18n.changeLanguage('ja');
    expect(
      i18n.t('tasks:actionDialogs.deleteIssues', {
        count: 1,
        formattedCount: '1',
      })
    ).toBe('1件のIssueを削除');
    expect(
      i18n.t('tasks:actionDialogs.deleteIssues', {
        count: 2,
        formattedCount: '2',
      })
    ).toBe('2件のIssueを削除');
  });
});

describe('archive action recovery identity', () => {
  it('retains request identity and re-translates a locally classified lost response', async () => {
    const error = new IssueArchiveActionError({
      kind: 'request',
      requestId: 'raw-request',
    });
    await i18n.changeLanguage('en');
    expect(actionErrorMessage(error)).toContain(
      'Unarchiving could not be confirmed'
    );
    expect(actionErrorMessage(error)).toContain('raw-request');
    await i18n.changeLanguage('ja');
    expect(actionErrorMessage(error)).toContain(
      'アーカイブ解除の結果を確認できませんでした'
    );
    expect(actionErrorMessage(error)).toContain('raw-request');
  });
  it('captures receipt identity without rendering provider bodies or accepting lookalikes', async () => {
    const receipt = {
      request_id: 'raw-request',
      status: 'uncertain',
      result: '{"message":"Raw secret"}',
    };
    const error = new IssueArchiveActionError({ kind: 'receipt', receipt });
    receipt.request_id = 'changed-request';
    receipt.result = '{}';
    for (const language of ['en', 'ja']) {
      await i18n.changeLanguage(language);
      expect(actionErrorMessage(error)).toContain('raw-request');
      expect(actionErrorMessage(error)).not.toContain('changed-request');
      expect(actionErrorMessage(error)).not.toContain('Raw secret');
      const spoof = {
        name: 'IssueArchiveActionError',
        notice: { kind: 'request', requestId: 'spoofed-request' },
      };
      expect(actionErrorMessage(spoof)).not.toContain('spoofed-request');
    }
  });
});
