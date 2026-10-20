import { describe, expect, it } from 'vitest';
import {
  BaseCodingAgent,
  ExecutionMode,
  type ExecutorConfig,
} from 'shared/types';
import {
  mergeExecutorOverrides,
  normaliseExecutionMode,
  resolveExecutorOverride,
} from './executorOverrides';

describe('immutable Session launch restoration', () => {
  const launch: ExecutorConfig = { executor: BaseCodingAgent.CODEX };
  const editedPreset: ExecutorConfig = {
    ...launch,
    reasoning_id: 'low',
    execution_mode: ExecutionMode.plan,
  };
  it('does not replace inherited values with a subsequently edited preset', () => {
    expect(
      resolveExecutorOverride('reasoning_id', {}, null, launch, editedPreset)
    ).toBeUndefined();
    expect(
      resolveExecutorOverride('execution_mode', {}, null, launch, editedPreset)
    ).toBeUndefined();
    expect(
      resolveExecutorOverride('reasoning_id', {}, null, null, editedPreset)
    ).toBe('low');
  });
  it('preserves explicit null, max/ultra, zero concurrency and user/draft edits', () => {
    expect(
      resolveExecutorOverride(
        'reasoning_id',
        { reasoning_id: null },
        null,
        editedPreset,
        null
      )
    ).toBeNull();
    for (const reasoning_id of ['max', 'ultra']) {
      expect(
        resolveExecutorOverride(
          'reasoning_id',
          {},
          { ...launch, reasoning_id },
          editedPreset,
          null
        )
      ).toBe(reasoning_id);
    }
    expect(
      resolveExecutorOverride(
        'goal_max_concurrent_agents',
        {},
        null,
        { ...launch, goal_max_concurrent_agents: 0 },
        null
      )
    ).toBe(0);
  });
});

describe('retired execution mode restoration', () => {
  it('launches old Plan with Goal drafts as Plan without mutating history', () => {
    const saved: ExecutorConfig = {
      executor: BaseCodingAgent.CODEX,
      execution_mode: ExecutionMode.plan_with_goal,
      goal_token_budget: 1000,
      goal_max_concurrent_agents: 2,
    };
    expect(normaliseExecutionMode(saved)).toEqual({
      ...saved,
      execution_mode: ExecutionMode.plan,
      goal_token_budget: null,
      goal_max_concurrent_agents: null,
    });
    expect(saved.execution_mode).toBe(ExecutionMode.plan_with_goal);
  });
  it('preserves Code, Plan and Goal settings', () => {
    for (const execution_mode of [
      ExecutionMode.code,
      ExecutionMode.plan,
      ExecutionMode.goal,
    ]) {
      const saved = { executor: BaseCodingAgent.CODEX, execution_mode };
      expect(normaliseExecutionMode(saved)).toBe(saved);
    }
  });
});

describe('manual override draft persistence', () => {
  const restored: ExecutorConfig = {
    executor: BaseCodingAgent.CODEX,
    model_id: 'custom/model',
    reasoning_id: 'max',
  };
  it('retains model identity when editing reasoning and reasoning when editing another field', () => {
    expect(mergeExecutorOverrides(restored, { reasoning_id: 'ultra' })).toEqual(
      { ...restored, reasoning_id: 'ultra' }
    );
    expect(
      mergeExecutorOverrides(restored, { execution_mode: ExecutionMode.plan })
    ).toEqual({ ...restored, execution_mode: ExecutionMode.plan });
  });
  it('preserves explicit Follow Settings and clears reasoning when changing model', () => {
    expect(
      mergeExecutorOverrides(restored, { reasoning_id: null }).reasoning_id
    ).toBeNull();
    expect(
      mergeExecutorOverrides(restored, { model_id: 'other' }).reasoning_id
    ).toBeNull();
    expect(
      mergeExecutorOverrides(restored, {
        model_id: 'other',
        reasoning_id: 'high',
      }).reasoning_id
    ).toBe('high');
    expect(restored.reasoning_id).toBe('max');
  });
});
