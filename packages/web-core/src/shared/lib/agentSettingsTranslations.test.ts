import { readFileSync } from 'node:fs';
import { createInstance } from 'i18next';
import { describe, expect, it } from 'vitest';
import en from '@/i18n/locales/en/settings.json';
import ja from '@/i18n/locales/ja/settings.json';
import type { SettingDescriptor } from 'shared/types';
import { parseSettingInput } from './agentSettingsModel';
import {
  agentSettingsSectionLabels,
  agentSettingsSourceLabel,
  agentSettingsExplanation,
  agentSettingsDiffText,
  agentSettingDescriptorText,
  agentSettingOptionLabel,
  agentSettingsErrorCode,
  agentSettingsErrorMessages,
  agentSettingsStateLabels,
  agentSettingValidationMessage,
} from './agentSettingsTranslations';

const descriptor = (
  control: SettingDescriptor['control'],
  validation = {}
): SettingDescriptor => ({
  key: { namespace: 'raw_namespace', name: 'raw_key' },
  section: 'general',
  label: 'Original provider label',
  description: '',
  control,
  value_type: control === 'json' ? 'string_map' : 'string',
  options: [{ value: 'RAW_VALUE', label: 'Original option' }],
  validation,
  supported_scopes: ['user'],
  capabilities: {
    readable: true,
    writable: true,
    resettable: true,
    profile_storable: true,
    run_override: true,
  },
  native_locations: [],
  activation: 'next_session',
});

describe('agent settings localized validation', () => {
  it('updates an existing error and section labels on language changes without reparsing the draft', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'en',
      fallbackLng: 'en',
      resources: { en: { settings: en }, ja: { settings: ja } },
    });
    const error = parseSettingInput(
      descriptor('number', { minimum: 2 }),
      '1'
    ).error;
    expect(
      agentSettingValidationMessage(i18n.getFixedT(null, 'settings'), error)
    ).toBe('Minimum value is 2.');
    expect(
      agentSettingsSectionLabels(i18n.getFixedT(null, 'settings')).native_files
    ).toBe('Native Files');
    await i18n.changeLanguage('ja');
    expect(
      agentSettingValidationMessage(i18n.getFixedT(null, 'settings'), error)
    ).toBe('2以上の値を入力してください。');
    expect(
      agentSettingsSectionLabels(i18n.getFixedT(null, 'settings')).native_files
    ).toBe('設定ファイル');
    expect(error).toEqual({ code: 'minimum', limit: 2 });
  });

  it('retains validation boundaries, numeric values, JSON, and raw select identities', () => {
    const text = descriptor('text', { max_length: 2, pattern: '^[A-Z]+$' });
    expect(parseSettingInput(text, 'ABC').error).toEqual({
      code: 'maxLength',
      limit: 2,
    });
    expect(parseSettingInput(text, 'aa').error).toEqual({ code: 'format' });
    expect(parseSettingInput(text, 'AB')).toEqual({ value: 'AB' });
    const number = descriptor('number', { minimum: 2, maximum: 4 });
    expect(parseSettingInput(number, '').error).toEqual({ code: 'number' });
    expect(parseSettingInput(number, 'Infinity').error).toEqual({
      code: 'number',
    });
    expect(parseSettingInput(number, '5').error).toEqual({
      code: 'maximum',
      limit: 4,
    });
    expect(parseSettingInput(number, '2')).toEqual({ value: 2 });
    expect(parseSettingInput(number, '4')).toEqual({ value: 4 });
    expect(parseSettingInput(descriptor('json'), '{').error).toEqual({
      code: 'json',
    });
    expect(parseSettingInput(descriptor('json'), '[]').error).toEqual({
      code: 'object',
    });
    expect(parseSettingInput(descriptor('json'), '{"原文":"RAW"}')).toEqual({
      value: { 原文: 'RAW' },
    });
    expect(parseSettingInput(descriptor('select'), '"RAW_VALUE"')).toEqual({
      value: 'RAW_VALUE',
    });
    expect(parseSettingInput(descriptor('select'), 'unknown').error).toEqual({
      code: 'option',
    });
  });
});

describe('agent settings safe operation errors', () => {
  it('only recognizes supported codes and never uses raw error bodies', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'en',
      fallbackLng: 'en',
      resources: { en: { settings: en }, ja: { settings: ja } },
    });
    for (const code of [
      'invalid_request',
      'invalid_configuration',
      'not_found',
      'stale_revision',
      'unsupported',
      'unsafe_path',
      'validation_failed',
      'io',
      'rollback_failed',
      'verification_failed',
    ]) {
      const category = agentSettingsErrorCode({
        error_data: {
          code,
          message: 'PRIVATE_BODY',
          recovery: 'PRIVATE_RECOVERY',
        },
      });
      expect(category).toBe(code);
      expect(
        agentSettingsErrorMessages(i18n.getFixedT(null, 'settings'))[category]
      ).not.toContain('PRIVATE');
    }
    for (const value of [
      new Error('PRIVATE_BODY'),
      null,
      { error_data: { code: 'future_code', message: 'PRIVATE_BODY' } },
    ])
      expect(agentSettingsErrorCode(value)).toBe('unconfirmed');
    const category = agentSettingsErrorCode({
      error_data: { code: 'stale_revision' },
    });
    expect(
      agentSettingsErrorMessages(i18n.getFixedT(null, 'settings'))[category]
    ).toContain('Refresh');
    await i18n.changeLanguage('ja');
    expect(
      agentSettingsErrorMessages(i18n.getFixedT(null, 'settings'))[category]
    ).toContain('再読み込み');
    expect(
      agentSettingsErrorMessages(i18n.getFixedT(null, 'settings'))
        .rollback_failed
    ).toContain('完全には復元できませんでした');
    expect(
      agentSettingsStateLabels(i18n.getFixedT(null, 'settings')).parse.parsed
    ).toBe('解析済み');
  });
});

describe('installed provider metadata translations', () => {
  it('covers every current Rust descriptor and select option while preserving unknown metadata', async () => {
    const source = readFileSync(
      new URL(
        '../../../../../crates/executors/src/agent_settings.rs',
        import.meta.url
      ),
      'utf8'
    );
    const i18n = createInstance();
    await i18n.init({
      lng: 'en',
      fallbackLng: 'en',
      resources: { en: { settings: en }, ja: { settings: ja } },
    });
    let descriptorCount = 0;
    let optionCount = 0;
    for (const [provider, start, end] of [
      ['codex', 'codex_descriptors', 'claude_descriptors'],
      ['claude_code', 'claude_descriptors', 'provider_limitations'],
    ]) {
      const text = source.slice(
        source.indexOf(`fn ${start}`),
        source.indexOf(`fn ${end}`)
      );
      const matches = [
        ...text.matchAll(
          /SettingKey::new\("([^"]+)", "([^"]+)"\),\s*SettingSection::\w+,\s*"([^"]+)",\s*"([^"]+)"/g
        ),
      ];
      for (let index = 0; index < matches.length; index++) {
        const match = matches[index];
        const descriptor = {
          key: { namespace: match[1], name: match[2] },
          label: match[3],
          description: match[4],
        };
        const original = JSON.stringify(descriptor);
        await i18n.changeLanguage('en');
        expect(
          agentSettingDescriptorText(
            i18n.getFixedT(null, 'settings'),
            provider,
            descriptor
          )
        ).toEqual({
          label: descriptor.label,
          description: descriptor.description,
        });
        await i18n.changeLanguage('ja');
        const translated = agentSettingDescriptorText(
          i18n.getFixedT(null, 'settings'),
          provider,
          descriptor
        );
        expect(translated.label).not.toBe(descriptor.label);
        expect(translated.description).not.toBe(descriptor.description);
        expect(JSON.stringify(descriptor)).toBe(original);
        descriptorCount++;
        const segment = text.slice(
          match.index! + match[0].length,
          matches[index + 1]?.index ?? text.length
        );
        for (const option of segment.matchAll(/\("([^"]+)", "([^"]+)"\)/g)) {
          const value = { value: option[1], label: option[2] };
          await i18n.changeLanguage('en');
          expect(
            agentSettingOptionLabel(
              i18n.getFixedT(null, 'settings'),
              provider,
              descriptor,
              value
            )
          ).toBe(value.label);
          await i18n.changeLanguage('ja');
          expect(
            agentSettingOptionLabel(
              i18n.getFixedT(null, 'settings'),
              provider,
              descriptor,
              value
            )
          ).not.toBe(value.label);
          expect(value.value).toBe(option[1]);
          optionCount++;
        }
      }
    }
    expect(descriptorCount).toBe(16);
    expect(optionCount).toBe(21);
    const unknown = {
      key: { namespace: 'future', name: 'field' },
      label: 'Raw label 原文',
      description: 'Raw description 原文',
    };
    expect(
      agentSettingDescriptorText(
        i18n.getFixedT(null, 'settings'),
        'codex',
        unknown
      )
    ).toEqual({ label: unknown.label, description: unknown.description });
    expect(
      agentSettingOptionLabel(
        i18n.getFixedT(null, 'settings'),
        'codex',
        unknown,
        { value: 'RAW', label: 'Raw option 原文' }
      )
    ).toBe('Raw option 原文');
  });
});

describe('owned explanations and native file boundaries', () => {
  it('translates known notes and sanitized diffs but preserves native bytes and unknown original notes', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'ja',
      fallbackLng: 'en',
      resources: { en: { settings: en }, ja: { settings: ja } },
    });
    const t = i18n.getFixedT(null, 'settings');
    expect(agentSettingsSourceLabel(t, 'native_user')).toBe('ユーザー設定');
    expect(agentSettingsSourceLabel(t, 'native_project')).toBe(
      'プロジェクト設定'
    );
    expect(agentSettingsSourceLabel(t, 'future_source')).toBe('future_source');
    expect(
      agentSettingsExplanation(
        t,
        'Persistent changes affect new provider sessions; active sessions are not restarted.'
      )
    ).toBe(
      '保存した変更は新しいプロバイダーセッションに反映されます。実行中のセッションは再起動しません。'
    );
    expect(agentSettingsExplanation(t, 'Raw future note 原文')).toBe(
      'プロバイダーの注意事項（原文）: Raw future note 原文'
    );
    const content = 'Observed native revision (values withheld)';
    expect(agentSettingsDiffText(t, content, false)).toBe(
      '確認済みの設定リビジョン（値は非表示）'
    );
    expect(agentSettingsDiffText(t, content, true)).toBe(content);
    expect(agentSettingsDiffText(t, 'No change.', true)).toBe('No change.');
    expect(agentSettingsDiffText(t, 'RAW=value 原文', false)).toBe(
      'RAW=value 原文'
    );
    expect(agentSettingsDiffText(t, '', true)).toBe('（未作成）');
    await i18n.changeLanguage('en');
    expect(agentSettingsDiffText(t, content, false)).toBe(content);
  });
});
