import { ExecutionMode, type ExecutorConfig } from 'shared/types';

/** A stored launch is a complete snapshot, not a patch over today's preset.
 * In particular absent/null reasoning means provider inheritance, not "low"
 * or the possibly edited preset. A draft, in contrast, may be a partial edit.
 */
export function resolveExecutorOverride<K extends keyof ExecutorConfig>(
  field: K,
  selection: Partial<ExecutorConfig>,
  scratch: ExecutorConfig | null | undefined,
  launch: ExecutorConfig | null | undefined,
  preset: ExecutorConfig | null | undefined
): ExecutorConfig[K] | undefined {
  if (Object.hasOwn(selection, field)) return selection[field];
  if (scratch && Object.hasOwn(scratch, field)) return scratch[field];
  if (launch) return launch[field];
  return preset?.[field];
}

/** Retain historical values in storage; a new launch of the retired mode is Plan. */
export function normaliseExecutionMode(config: ExecutorConfig): ExecutorConfig {
  if (config.execution_mode !== ExecutionMode.plan_with_goal) return config;
  return {
    ...config,
    execution_mode: ExecutionMode.plan,
    goal_token_budget: null,
    goal_max_concurrent_agents: null,
  };
}

/** Save edits through the existing draft path as a complete selection. */
export function mergeExecutorOverrides(
  config: ExecutorConfig,
  partial: Partial<ExecutorConfig>
): ExecutorConfig {
  const next = { ...config, ...partial };
  if ('model_id' in partial && !('reasoning_id' in partial)) {
    // Explicit inheritance blocks the previous launch/preset from resurfacing.
    next.reasoning_id = null;
  }
  return next;
}
