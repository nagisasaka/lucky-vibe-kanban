import { createInstance } from 'i18next';
import { describe, expect, it } from 'vitest';
import validator from '@rjsf/validator-ajv8';
import type { RJSFSchema } from '@rjsf/utils';
import codex from '../../../../../shared/schemas/codex.json';
import claude from '../../../../../shared/schemas/claude_code.json';
import en from '@/i18n/locales/en/settings.json';
import ja from '@/i18n/locales/ja/settings.json';
import {
  localizeExecutorSchema,
  executorSchemaUiOptions,
  localizeSchemaErrors,
} from './executorSchemaLocalization';

async function instance() {
  const i18n = createInstance();
  await i18n.init({
    lng: 'ja',
    fallbackLng: 'en',
    resources: { en: { settings: en }, ja: { settings: ja } },
  });
  return i18n;
}
function semantics(schema: RJSFSchema) {
  const copy = structuredClone(schema);
  for (const property of Object.values(copy.properties ?? {})) {
    if (typeof property === 'object') {
      delete property.title;
      delete property.description;
    }
  }
  return copy;
}

describe('generated executor schema localization', () => {
  for (const [name, schema] of [
    ['codex', codex],
    ['claude_code', claude],
  ] as const) {
    it(`${name}: translates every generated field without changing validation or data`, async () => {
      const i18n = await instance();
      const original = structuredClone(schema);
      const localized = localizeExecutorSchema(
        schema as RJSFSchema,
        i18n.getFixedT(null, 'settings')
      );
      expect(semantics(localized)).toEqual(semantics(schema as RJSFSchema));
      expect(schema).toEqual(original);
      expect(localized.properties?.model).toMatchObject({ title: 'モデル' });
      for (const [field, property] of Object.entries(
        localized.properties ?? {}
      )) {
        if (typeof property !== 'object') continue;
        expect(property.title).toBe(
          ja.executorSchemas[name][
            field as keyof (typeof ja.executorSchemas)[typeof name]
          ].title
        );
        expect(property.title).not.toContain('executorSchemas.');
      }
      for (const data of [
        {},
        { goal_token_budget: -1 },
        { model: 'Raw model 原文', env: { RAW_KEY: '原文' } },
        { model: 12 },
      ]) {
        const before = structuredClone(data);
        const raw = validator.rawValidation(schema as RJSFSchema, data);
        const translated = validator.rawValidation(localized, data);
        const diagnostics = (errors: typeof raw.errors) =>
          errors?.map(
            ({ instancePath, schemaPath, keyword, params, message }) => ({
              instancePath,
              schemaPath,
              keyword,
              params,
              message,
            })
          );
        expect(diagnostics(translated.errors)).toEqual(diagnostics(raw.errors));
        expect(data).toEqual(before);
      }
      await i18n.changeLanguage('en');
      expect(
        localizeExecutorSchema(
          schema as RJSFSchema,
          i18n.getFixedT(null, 'settings')
        )
      ).toEqual(schema);
    });
  }
  it('changes stored validation messages with locale while preserving diagnostic identities', async () => {
    const i18n = await instance();
    const errors = [
      {
        name: 'minimum',
        property: '.goal_token_budget',
        message: 'raw internal prose',
        params: { limit: 0 },
        stack: 'raw',
      },
      { name: 'future_keyword', property: '.raw', params: {}, stack: 'raw' },
    ];
    expect(
      localizeSchemaErrors(errors, i18n.getFixedT(null, 'settings'))[0]
    ).toMatchObject({
      message: '0以上の値を入力してください。',
      property: '.goal_token_budget',
      params: { limit: 0 },
    });
    expect(
      localizeSchemaErrors(errors, i18n.getFixedT(null, 'settings'))[1].message
    ).toBe('値を確認してください。');
    await i18n.changeLanguage('en');
    expect(
      localizeSchemaErrors(errors, i18n.getFixedT(null, 'settings'))[0].message
    ).toBe('Enter a value of at least 0.');
    expect(errors[0].message).toBe('raw internal prose');
  });
});

it('labels all enum choices while retaining canonical values and distinct null inheritance', async () => {
  const i18n = await instance();
  let count = 0;
  for (const schema of [codex, claude]) {
    const before = structuredClone(schema);
    const ui = executorSchemaUiOptions(
      schema as RJSFSchema,
      i18n.getFixedT(null, 'settings')
    );
    for (const [name, property] of Object.entries(schema.properties)) {
      if (!('enum' in property)) continue;
      const labels = ui[name]['ui:enumNames'];
      expect(labels).toHaveLength(property.enum.length);
      expect(labels.every((label) => !label.includes('executorSchemas.'))).toBe(
        true
      );
      if (property.enum.includes(null))
        expect(labels[property.enum.indexOf(null)]).toBe('設定ファイルに従う');
      if (property.enum.includes('none'))
        expect(labels[property.enum.indexOf('none')]).toBe('なし');
      count += labels.length;
    }
    expect(schema).toEqual(before);
  }
  expect(count).toBe(28);
  expect(
    executorSchemaUiOptions(
      codex as RJSFSchema,
      i18n.getFixedT(null, 'settings')
    ).sandbox['ui:enumNames'][2]
  ).toBe('ワークスペースへの書き込み');
  await i18n.changeLanguage('en');
  expect(
    executorSchemaUiOptions(
      codex as RJSFSchema,
      i18n.getFixedT(null, 'settings')
    ).sandbox['ui:enumNames'][2]
  ).toBe('Workspace write');
});
