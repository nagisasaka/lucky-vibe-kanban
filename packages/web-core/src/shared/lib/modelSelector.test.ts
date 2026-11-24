import { describe, expect, it } from 'vitest';
import type { ModelInfo } from 'shared/types';
import {
  buildModelSelectionOverride,
  parseModelId,
  resolveConfiguredReasoningIdForOptions,
  resolveDefaultReasoningId,
  resolveReasoningIdForOptions,
} from './modelSelector';

const models: ModelInfo[] = [
  {
    id: 'gpt-5.5',
    name: 'GPT-5.5',
    provider_id: null,
    reasoning_options: [
      { id: 'medium', label: 'Medium', is_default: false },
      { id: 'xhigh', label: 'Extra High', is_default: true },
    ],
  },
  {
    id: 'sonnet',
    name: 'Sonnet',
    provider_id: 'anthropic',
    reasoning_options: [
      { id: 'medium', label: 'Medium', is_default: false },
      { id: 'high', label: 'High', is_default: true },
    ],
  },
  {
    id: 'haiku',
    name: 'Haiku',
    provider_id: 'anthropic',
    reasoning_options: [],
  },
];

describe('model selector model ID semantics', () => {
  it('preserves colons inside model IDs', () => {
    expect(parseModelId('custom:lfm2.5:8b', true)).toEqual({
      providerId: null,
      modelId: 'custom:lfm2.5:8b',
    });
  });

  it('splits only the first slash when providers exist', () => {
    expect(parseModelId('custom/model/family:latest', true)).toEqual({
      providerId: 'custom',
      modelId: 'model/family:latest',
    });
  });

  it.each(['custom:lfm2.5:8b', 'model/family:latest'])(
    'round-trips provider-scoped model ID %s without rewriting it',
    (modelId) => {
      const { model_id: value } = buildModelSelectionOverride(
        models,
        modelId,
        'custom'
      );

      expect(value).toBe(`custom/${modelId}`);
      expect(parseModelId(value, true)).toEqual({
        providerId: 'custom',
        modelId,
      });
    }
  );

  it('treats the entire value as a model ID when providers are absent', () => {
    const value = 'custom/model/family:latest';

    expect(parseModelId(value, false)).toEqual({
      providerId: null,
      modelId: value,
    });
  });
});

describe('model selector reasoning overrides', () => {
  it('does not write discovery default reasoning when selecting a model', () => {
    expect(buildModelSelectionOverride(models, 'gpt-5.5')).toEqual({
      model_id: 'gpt-5.5',
    });
  });

  it('includes provider-scoped model ids without reasoning overrides', () => {
    expect(buildModelSelectionOverride(models, 'sonnet', 'anthropic')).toEqual({
      model_id: 'anthropic/sonnet',
    });
  });

  it('returns only the model override when the model has no reasoning options', () => {
    expect(buildModelSelectionOverride(models, 'haiku', 'anthropic')).toEqual({
      model_id: 'anthropic/haiku',
    });
  });

  it('keeps the discovery default available only as metadata', () => {
    expect(resolveDefaultReasoningId(models[0].reasoning_options)).toBe(
      'xhigh'
    );
  });

  it('does not invent a default reasoning option from list order', () => {
    expect(
      resolveDefaultReasoningId([
        { id: 'medium', label: 'Medium', is_default: false },
        { id: 'high', label: 'High', is_default: false },
      ])
    ).toBeNull();
  });

  it('validates preferred reasoning without falling back to the model default', () => {
    expect(
      resolveReasoningIdForOptions(models[0].reasoning_options, 'medium')
    ).toBe('medium');
    expect(
      resolveReasoningIdForOptions(models[0].reasoning_options, 'low')
    ).toBeNull();
    expect(
      resolveReasoningIdForOptions(models[0].reasoning_options, null)
    ).toBeNull();
  });

  it('resolves only configured reasoning for display and send semantics', () => {
    expect(
      resolveConfiguredReasoningIdForOptions(
        models[0].reasoning_options,
        'xhigh',
        true
      )
    ).toBe('xhigh');
    expect(
      resolveConfiguredReasoningIdForOptions(
        models[0].reasoning_options,
        'xhigh',
        false
      )
    ).toBeNull();
    expect(
      resolveConfiguredReasoningIdForOptions(
        models[0].reasoning_options,
        'low',
        true
      )
    ).toBe('low');
    expect(
      resolveConfiguredReasoningIdForOptions(
        models[0].reasoning_options,
        null,
        true
      )
    ).toBeNull();
  });

  it.each(['max', 'ultra', 'custom-effort'])(
    'retains explicit %s across discovery gaps',
    (effort) => {
      for (const options of [[], models[0].reasoning_options]) {
        expect(
          resolveConfiguredReasoningIdForOptions(options, effort, true)
        ).toBe(effort);
        expect(
          resolveConfiguredReasoningIdForOptions(options, null, true)
        ).toBeNull();
        expect(
          resolveConfiguredReasoningIdForOptions(options, undefined, false)
        ).toBeNull();
      }
    }
  );
});

import { BaseCodingAgent, type ModelSelectorConfig } from 'shared/types';
import { resolveRunSelection } from './modelSelector';
import {
  resolveExecutorOverride,
  mergeExecutorOverrides,
} from './executorOverrides';

describe('read-only inherited run selection', () => {
  const config: ModelSelectorConfig = {
    models,
    providers: [],
    agents: [],
    permissions: [],
    run_defaults: {
      model: 'gpt-5.5',
      reasoning: 'xhigh',
      approval: 'never',
      model_source: 'configured',
      reasoning_source: 'provider',
    },
  };
  const empty = { executor: BaseCodingAgent.CODEX };
  it('displays resolved values without turning them or profile values into overrides', () => {
    expect(resolveRunSelection(config, empty, null)).toMatchObject({
      model: 'gpt-5.5',
      reasoning: 'xhigh',
      modelOverridden: false,
      reasoningOverridden: false,
    });
    const preset = { ...empty, model_id: 'gpt-5.5', reasoning_id: 'xhigh' };
    for (const field of [
      'model_id',
      'reasoning_id',
      'permission_policy',
    ] as const) {
      expect(
        resolveExecutorOverride(field, {}, null, null, preset)
      ).toBeUndefined();
    }
    expect(empty).toEqual({ executor: BaseCodingAgent.CODEX });
  });
  it('marks only changed fields, and removes emphasis on same value or reset', () => {
    expect(
      resolveRunSelection(config, { ...empty, reasoning_id: 'max' }, null)
    ).toMatchObject({ modelOverridden: false, reasoningOverridden: true });
    for (const reasoning_id of ['xhigh', null]) {
      expect(
        resolveRunSelection(config, { ...empty, reasoning_id }, null)
          .reasoningOverridden
      ).toBe(false);
    }
    expect(
      resolveRunSelection(config, { ...empty, model_id: 'gpt-5.5' }, null)
        .modelOverridden
    ).toBe(false);
  });
  it('clears old reasoning when changing model and does not guess a new effort', () => {
    const next = mergeExecutorOverrides(
      { ...empty, reasoning_id: 'xhigh' },
      { model_id: 'haiku' }
    );
    expect(next.reasoning_id).toBeNull();
    expect(resolveRunSelection(config, next, null)).toMatchObject({
      model: 'haiku',
      reasoning: null,
      reasoningUnsupported: true,
      modelOverridden: true,
      reasoningOverridden: false,
    });
  });
  it('preserves explicit unknown values and distinguishes unverified resume from candidates', () => {
    const resumed = {
      ...config,
      run_defaults: {
        model: null,
        reasoning: null,
        approval: null,
        model_source: 'resume',
        reasoning_source: 'resume',
      },
    };
    expect(resolveRunSelection(resumed, empty, null)).toMatchObject({
      model: null,
      reasoning: null,
      modelSource: 'resume',
      modelOverridden: false,
    });
    expect(
      resolveRunSelection(
        resumed,
        { ...empty, model_id: 'custom', reasoning_id: 'ultra' },
        null
      )
    ).toMatchObject({
      model: 'custom',
      reasoning: 'ultra',
      modelOverridden: true,
      reasoningOverridden: true,
    });
  });
  it('a saved reset does not resurrect a previous launch override', () => {
    const saved = { ...empty, execution_mode: undefined };
    const previous = { ...empty, model_id: 'custom', reasoning_id: 'ultra' };
    expect(
      resolveExecutorOverride('model_id', {}, saved, previous, null)
    ).toBeNull();
    expect(
      resolveExecutorOverride('reasoning_id', {}, saved, previous, null)
    ).toBeNull();
  });
});

it('does not display native default-model effort on a different profile model', () => {
  const config: ModelSelectorConfig = {
    models,
    providers: [],
    agents: [],
    permissions: [],
    run_defaults: {
      model: 'gpt-5.5',
      reasoning: 'xhigh',
      approval: null,
      model_source: 'configured',
      reasoning_source: 'configured',
    },
  };
  expect(
    resolveRunSelection(
      config,
      { executor: BaseCodingAgent.CODEX },
      { executor: BaseCodingAgent.CODEX, model_id: 'haiku' }
    )
  ).toMatchObject({
    model: 'haiku',
    reasoning: null,
    reasoningUnsupported: true,
    modelOverridden: false,
  });
});
