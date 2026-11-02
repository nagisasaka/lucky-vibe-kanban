import { createInstance } from 'i18next';
import { expect, it } from 'vitest';
import en from '@/i18n/locales/en/common.json';
import ja from '@/i18n/locales/ja/common.json';
import { sequentialShortcutLabel } from './labels';
import {
  sequentialBindings,
  keyBindings,
  getKeysFor,
  Action,
  Scope,
} from './registry';

it('resolves every sequential label in EN/JA without modifying bindings on locale changes', async () => {
  const i18n = createInstance();
  await i18n.init({
    lng: 'en',
    fallbackLng: 'en',
    resources: { en: { common: en }, ja: { common: ja } },
  });
  const before = structuredClone({ sequentialBindings, keyBindings });
  for (const binding of sequentialBindings) {
    const english = sequentialShortcutLabel(
      i18n.getFixedT('en', 'common'),
      binding.actionId
    );
    const japanese = sequentialShortcutLabel(
      i18n.getFixedT('ja', 'common'),
      binding.actionId
    );
    expect(english).toBe(en.shortcuts.actions[binding.actionId]);
    expect(japanese).toBe(ja.shortcuts.actions[binding.actionId]);
    expect(japanese).not.toBe(english);
    expect(japanese).not.toContain('shortcuts.actions.');
  }
  await i18n.changeLanguage('ja');
  expect({ sequentialBindings, keyBindings }).toEqual(before);
  expect(getKeysFor(Action.SUBMIT_FOLLOW_UP, Scope.FOLLOW_UP_READY)).toEqual([
    'meta+enter',
  ]);
  expect(
    sequentialBindings.find((binding) => binding.actionId === 'settings')?.keys
  ).toEqual(['g', 's']);
});
