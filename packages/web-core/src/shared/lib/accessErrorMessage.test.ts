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
    expect(accessErrorMessage('Invalid Auth0 issuer', 400)).toContain(expected);
  });

  it('uses English fallback for another UI locale', async () => {
    await i18n.changeLanguage('ja');
    const pending = accessRequestFailure('Invalid Auth0 issuer', 400);
    expect(accessErrorNotice(pending)).toContain(
      'Auth0 issuerが正しくありません。'
    );
    await i18n.changeLanguage('fr');
    expect(accessErrorNotice(pending)).toBe('Invalid Auth0 issuer');
    expect(accessErrorMessage('Invalid Auth0 issuer', 400)).toBe(
      'Invalid Auth0 issuer'
    );
  });

  it('keeps original diagnostics alongside translated explanations', async () => {
    await i18n.changeLanguage('ja');
    const detail = 'provider raw diagnostic /internal/path';
    const pending = accessRequestFailure(detail, 502);
    expect(accessErrorNotice(pending)).toContain(detail);
    expect(accessErrorNotice(pending)).toContain(
      i18n.t('auth.errors.requestFailed', { ns: 'settings' })
    );
    expect(pending.message).toBe(detail);
  });

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
