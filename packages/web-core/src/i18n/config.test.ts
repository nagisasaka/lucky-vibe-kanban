import LanguageDetector from 'i18next-browser-languagedetector';
import { createInstance } from 'i18next';
import { afterEach, describe, expect, it, vi } from 'vitest';
import configuredI18n, { detectBrowserNotificationLanguage } from './config';

const languages = [
  'fr',
  'es',
  'ko',
  'zh-Hans',
  'zh-Hant',
  'zh-TW',
  'zh-HK',
  'zh-MO',
  'zh',
];

afterEach(() => vi.unstubAllGlobals());

describe('shipped UI dictionaries', () => {
  it.each(languages)(
    'falls back to English for missing %s keys',
    async (lng) => {
      const instance = createInstance();
      await instance.init({
        ...configuredI18n.options,
        debug: false,
        lng,
        resources: {
          ...configuredI18n.options.resources,
          en: {
            ...configuredI18n.options.resources?.en,
            fallbackTest: { message: 'English fallback' },
          },
        },
      });
      expect(instance.t('fallbackTest:message')).toBe('English fallback');
    }
  );

  it.each([
    ['en-US', 'en'],
    ['ja-JP', 'ja'],
    ['zh-TW', 'zh-Hant'],
    ['zh-HK', 'zh-Hant'],
    ['zh-MO', 'zh-Hant'],
    ['zh', 'zh-Hans'],
  ])(
    'detects browser %s using the shipped %s dictionary',
    async (language, expected) => {
      vi.stubGlobal('navigator', { languages: [language], language });
      const instance = createInstance().use(LanguageDetector);
      await instance.init({
        ...configuredI18n.options,
        debug: false,
        lng: undefined,
      });
      expect(instance.resolvedLanguage).toBe(expected);
      expect(instance.t('buttons.cancel')).toBe(
        configuredI18n.getResource(expected, 'common', 'buttons.cancel')
      );
    }
  );

  it('keeps the first supported browser preference rather than forcing English', async () => {
    vi.stubGlobal('navigator', {
      languages: ['de-DE', 'ja-JP'],
      language: 'de-DE',
    });
    const instance = createInstance().use(LanguageDetector);
    await instance.init({
      ...configuredI18n.options,
      debug: false,
      lng: undefined,
    });
    expect(instance.resolvedLanguage).toBe('ja');
    expect(instance.t('buttons.cancel')).toBe('キャンセル');
  });

  it('falls back to English for an empty translated value', async () => {
    const instance = createInstance();
    await instance.init({
      ...configuredI18n.options,
      debug: false,
      lng: 'ja',
      resources: {
        en: { common: { value: 'English value' } },
        ja: { common: { value: '' } },
      },
    });
    expect(instance.t('value')).toBe('English value');
  });

  it('renders Japanese and English and updates the document language', async () => {
    const documentElement = { lang: '' };
    vi.stubGlobal('document', { documentElement });
    await configuredI18n.changeLanguage('ja');
    expect(configuredI18n.t('agentSessionResume.loading')).toBe(
      'セッションを読み込み中…'
    );
    expect(documentElement.lang).toBe('ja');
    await configuredI18n.changeLanguage('en');
    expect(configuredI18n.t('agentSessionResume.loading')).toBe(
      'Loading sessions...'
    );
    expect(documentElement.lang).toBe('en');
  });
});

it.each([
  [['ja-JP'], 'ja'],
  [['en-US'], 'en'],
  [['de-DE', 'ja-JP'], 'ja'],
  [['fr-FR', 'ja-JP'], 'en'],
  [['zh-TW'], 'en'],
] as const)(
  'uses the same first-supported browser preference for notification hints (%s)',
  (languages, expected) => {
    vi.stubGlobal('navigator', {
      languages: [...languages],
      language: languages[0],
    });
    const before = configuredI18n.language;
    expect(detectBrowserNotificationLanguage()).toBe(expected);
    expect(configuredI18n.language).toBe(before);
  }
);
