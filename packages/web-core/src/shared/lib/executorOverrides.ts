import { ExecutionMode, type ExecutorConfig } from 'shared/types';

/** A stored launch is a complete snapshot, not a patch over today's preset.
 * In particular absent/null reasoning means provider inheritance, not "low"
 * or the possibly edited preset. Other draft fields may be partial edits;
 * identity-only drafts select a preset; override snapshots may contain a
 * serialized None reasoning override.
 */
export function resolveExecutorOverride<K extends keyof ExecutorConfig>(
  field: K,
  selection: Partial<ExecutorConfig>,
  scratch: ExecutorConfig | null | undefined,
  launch: ExecutorConfig | null | undefined,
  preset: ExecutorConfig | null | undefined
): ExecutorConfig[K] | undefined {
  if (Object.hasOwn(selection, field)) return selection[field];
  if (scratch) {
    // setExecutor/setVariant persist only identity and expect preset fallback.
    // setOverrides persists the effective selection. In that snapshot Rust
    // omits None reasoning, which must block the previous effort on restore.
    const hasOverrides = Object.keys(scratch).some(
      (key) => key !== 'executor' && key !== 'variant'
    );
    const draft: ExecutorConfig = hasOverrides
      ? {
          model_id: null,
          reasoning_id: null,
          permission_policy: null,
          ...scratch,
        }
      : scratch;
    if (Object.hasOwn(draft, field)) return draft[field];
  }
  if (launch) return launch[field];
  // Display inheritance separately. The adapter already applies the selected
  // profile; copying its display values here turns a read into an override.
  if (
    field === 'model_id' ||
    field === 'reasoning_id' ||
    field === 'permission_policy'
  )
    return undefined;
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
  if (
    'model_id' in partial &&
    (partial.model_id ?? null) !== (config.model_id ?? null) &&
    !('reasoning_id' in partial)
  ) {
    // Explicit inheritance blocks the previous launch/preset from resurfacing.
    next.reasoning_id = null;
  }
  return next;
}
