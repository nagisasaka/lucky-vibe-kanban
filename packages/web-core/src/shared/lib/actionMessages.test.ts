import { it, expect } from 'vitest';
import i18n from '@/i18n/config';
import { actionErrorMessage } from './actionMessages';
import { IssueArchiveActionError } from './issueArchiveNotice';

it('re-translates recovery guidance without rewriting the request ID or diagnostic', async () => {
  const error = new IssueArchiveActionError(
    { kind: 'request', requestId: 'raw-request' },
    'Original diagnostic'
  );
  try {
    for (const language of ['ja', 'en']) {
      await i18n.changeLanguage(language);
      const text = actionErrorMessage(error);
      expect(text).toContain('raw-request');
      expect(text).toContain('Original diagnostic');
      expect(text).toContain(
        language === 'ja'
          ? 'アーカイブ解除の結果を確認できませんでした'
          : 'Unarchiving could not be confirmed'
      );
    }
  } finally {
    await i18n.changeLanguage('en');
  }
});
