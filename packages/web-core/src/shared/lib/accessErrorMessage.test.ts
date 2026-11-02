import { afterEach, describe, expect, it } from 'vitest';
import i18n from '@/i18n/config';
import {
  accessErrorMessage,
  accessRequestFailure,
  accessErrorNotice,
} from './accessErrorMessage';

afterEach(async () => {
  await i18n.changeLanguage('en');
});

describe('authentication UI errors', () => {
  it.each([
    ['en', 'Invalid Auth0 issuer'],
    ['ja', 'Auth0 issuerが正しくありません。'],
  ])('translates owned validation errors in %s', async (language, expected) => {
    await i18n.changeLanguage(language);
    expect(accessErrorMessage('Invalid Auth0 issuer', 400)).toBe(expected);
  });

  it('uses English fallback for another UI locale', async () => {
    await i18n.changeLanguage('ja');
    const pending = accessRequestFailure('Invalid Auth0 issuer', 400);
    expect(accessErrorNotice(pending)).toBe('Auth0 issuerが正しくありません。');
    await i18n.changeLanguage('fr');
    expect(accessErrorNotice(pending)).toBe('Invalid Auth0 issuer');
    expect(accessErrorMessage('Invalid Auth0 issuer', 400)).toBe(
      'Invalid Auth0 issuer'
    );
  });

  it.each(['en', 'ja'])(
    'keeps unknown provider/server content out of the %s UI',
    async (language) => {
      await i18n.changeLanguage(language);
      const message = accessErrorMessage(
        'provider raw body contains private-token and internal/path',
        502
      );
      expect(message).toBe(
        i18n.t('auth.errors.requestFailed', { ns: 'settings' })
      );
      const pending = accessRequestFailure('private-token /internal/path', 502);
      expect(accessErrorNotice(pending)).toBe(message);
      expect(pending.message).not.toContain('private-token');
      expect(message).not.toContain('private-token');
      expect(message).not.toContain('internal/path');
      expect(message).not.toContain('auth.errors.');
      for (const inherited of ['constructor', '__proto__', 'toString'])
        expect(accessErrorMessage(inherited, 502)).toBe(message);
    }
  );

  it('resolves status fallbacks in Japanese', async () => {
    await i18n.changeLanguage('ja');
    expect(accessErrorMessage(undefined, 401)).toBe('ログインが必要です。');
    expect(accessErrorMessage({}, 403)).toBe(
      'この操作を行う権限がありません。'
    );
    expect(accessErrorMessage(null, 429)).toBe(
      '認証の試行回数が多すぎます。1分後に操作してください。'
    );
  });
});
