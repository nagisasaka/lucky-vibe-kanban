import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckIcon,
  FastForwardIcon,
  MagnifyingGlassIcon,
  GearIcon,
  HandIcon,
  ListBulletsIcon,
  SlidersHorizontalIcon,
  TargetIcon,
  type Icon,
} from '@phosphor-icons/react';
import type { ExecutorConfig, ModelInfo } from 'shared/types';
import { BaseCodingAgent, ExecutionMode, PermissionPolicy } from 'shared/types';
import { toPrettyCase } from '@/shared/lib/string';
import {
  getModelKey,
  getRecentModelEntries,
  touchRecentModel,
  updateRecentModelEntries,
  setRecentReasoning,
} from '@/shared/lib/recentModels';
import {
  getReasoningLabel,
  getSelectedModel,
  escapeAttributeValue,
  parseModelId,
  appendPresetModel,
  buildModelSelectionOverride,
  findModelForSelection,
  resolveRunSelection,
} from '@/shared/lib/modelSelector';
import { profilesApi } from '@/shared/lib/api';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { getResolvedTheme, useTheme } from '@/shared/hooks/useTheme';
import {
  useModelSelectorConfig,
  type ExecutorDiscoverySnapshot,
} from '@/shared/hooks/useExecutorDiscovery';
import { ModelSelectorPopover } from '@vibe/ui/components/ModelSelectorPopover';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTriggerButton,
} from '@vibe/ui/components/Dropdown';

interface ModelSelectorContainerProps {
  discovery?: ExecutorDiscoverySnapshot;
  agent: BaseCodingAgent | null;
  workspaceId: string | undefined;
  sessionId?: string;
  repoId?: string;
  onAdvancedSettings: () => void;
  presets: string[];
  selectedPreset: string | null;
  onPresetSelect: (presetId: string | null) => void;
  onOverrideChange: (partial: Partial<ExecutorConfig>) => void;
  executorConfig: ExecutorConfig | null;
  presetOptions: ExecutorConfig | null | undefined;
}

const EMPTY_REASONING_OPTIONS: ModelInfo['reasoning_options'] = [];

type PendingReasoningSelection = {
  reasoningId: string | null;
};

export function ModelSelectorContainer({
  discovery,
  agent,
  workspaceId,
  sessionId,
  repoId,
  onAdvancedSettings,
  presets,
  selectedPreset,
  onPresetSelect,
  onOverrideChange,
  executorConfig,
  presetOptions,
}: ModelSelectorContainerProps) {
  const { t } = useTranslation('common');
  const { theme } = useTheme();
  const resolvedTheme = getResolvedTheme(theme);
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedProviderId, setExpandedProviderId] = useState('');
  const { profiles, setProfiles, reloadSystem } = useUserSystem();
  const defaultLabel = t('modelSelector.default');
  const loadingLabel = t('states.loading');

  const permissionMetaByPolicy: Record<
    PermissionPolicy,
    { label: string; icon: Icon }
  > = {
    [PermissionPolicy.AUTO]: {
      label: t(
        agent === BaseCodingAgent.CODEX
          ? 'modelSelector.approvalNever'
          : 'modelSelector.permissionAuto'
      ),
      icon: FastForwardIcon,
    },
    [PermissionPolicy.SUPERVISED]: {
      label: t(
        agent === BaseCodingAgent.CODEX
          ? 'modelSelector.approvalUntrusted'
          : 'modelSelector.permissionAsk'
      ),
      icon: HandIcon,
    },
    [PermissionPolicy.ON_REQUEST]: {
      label: t('modelSelector.approvalOnRequest'),
      icon: HandIcon,
    },
    [PermissionPolicy.PLAN]: {
      label: t('modelSelector.permissionPlan'),
      icon: ListBulletsIcon,
    },
  };

  const resolvedPreset =
    selectedPreset ??
    (presets.includes('DEFAULT') ? 'DEFAULT' : (presets[0] ?? null));

  const {
    config: streamConfig,
    loadingModels,
    error: streamError,
  } = useModelSelectorConfig(
    agent,
    {
      variant: resolvedPreset,
      workspaceId,
      sessionId,
      repoId,
    },
    discovery
  );

  useEffect(() => {
    if (streamError) {
      console.error('Failed to fetch model config', streamError);
    }
  }, [streamError]);

  const inheritedModel =
    presetOptions?.model_id ?? streamConfig?.run_defaults?.model;
  const selectedModelProbe =
    agent === BaseCodingAgent.CLAUDE_CODE &&
    executorConfig?.model_id &&
    executorConfig.model_id !== inheritedModel
      ? executorConfig.model_id
      : undefined;
  const selectedDiscovery = useModelSelectorConfig(
    selectedModelProbe ? agent : null,
    {
      variant: resolvedPreset,
      workspaceId,
      sessionId,
      repoId,
      selectedModel: selectedModelProbe,
    }
  );
  // Keep baseline defaults (and override comparisons) independent of this
  // request. The stream hook discards results for previous endpoint keys.
  const baseConfig = useMemo(() => {
    if (!streamConfig || !selectedModelProbe || !selectedDiscovery.config)
      return streamConfig;
    const selected = selectedDiscovery.config;
    const effort = selected.run_defaults?.reasoning;
    return {
      ...streamConfig,
      models: streamConfig.models.map((row) => {
        if (row.id !== selectedModelProbe) return row;
        const native = selected.models.find((m) => m.id === selectedModelProbe);
        const options = [
          ...(native?.reasoning_options ?? row.reasoning_options),
        ];
        if (effort && !options.some((r) => r.id === effort)) {
          options.push({
            id: effort,
            label: toPrettyCase(effort),
            is_default: true,
          });
        }
        return {
          ...row,
          reasoning_options: options.map((r) => ({
            ...r,
            is_default: effort ? r.id === effort : r.is_default,
          })),
        };
      }),
      unsupported_reasoning_models: [
        ...new Set([
          ...(streamConfig.unsupported_reasoning_models ?? []),
          ...(selected.unsupported_reasoning_models ?? []),
        ]),
      ],
    };
  }, [streamConfig, selectedModelProbe, selectedDiscovery.config]);
  const selectedModelLoading =
    !!selectedModelProbe &&
    !selectedDiscovery.isInitialized &&
    !selectedDiscovery.error;
  const config = appendPresetModel(baseConfig, presetOptions?.model_id);

  const availableProviderIds = useMemo(
    () => config?.providers.map((item) => item.id) ?? [],
    [config?.providers]
  );
  const hasProviders = availableProviderIds.length > 0;
  const selection = resolveRunSelection(
    baseConfig,
    executorConfig,
    presetOptions
  );
  const { providerId: selectedProviderId, modelId: selectedModelId } =
    parseModelId(selection.model, hasProviders);
  const selectedReasoningId = selection.reasoning;

  const defaultAgentId =
    config?.agents.find((entry) => entry.is_default)?.id ?? null;

  const selectedAgentId =
    executorConfig?.agent_id !== undefined
      ? executorConfig.agent_id
      : (presetOptions?.agent_id ?? defaultAgentId);

  const supportsPermissions = (config?.permissions.length ?? 0) > 0;

  const nativeApproval = config?.run_defaults?.approval;
  const nativePolicy =
    nativeApproval === 'never'
      ? PermissionPolicy.AUTO
      : nativeApproval === 'on-request'
        ? PermissionPolicy.ON_REQUEST
        : nativeApproval === 'untrusted'
          ? PermissionPolicy.SUPERVISED
          : null;
  const basePermissionPolicy = supportsPermissions
    ? (presetOptions?.permission_policy ??
      (agent === BaseCodingAgent.CODEX
        ? nativePolicy
        : config?.permissions[0]) ??
      null)
    : null;
  const permissionOverridden =
    executorConfig?.permission_policy != null &&
    executorConfig.permission_policy !== basePermissionPolicy;
  const permissionPolicy = supportsPermissions
    ? (executorConfig?.permission_policy ?? basePermissionPolicy)
    : null;

  // LRU persistence (on popover close)

  const recentModelEntries = getRecentModelEntries(profiles, agent);
  const pendingModelRef = useRef<ModelInfo | null>(null);
  const pendingReasoningRef = useRef<PendingReasoningSelection | null>(null);

  const persistPendingSelections = useCallback(() => {
    if (!profiles || !agent) return;
    if (!pendingModelRef.current && pendingReasoningRef.current === null) {
      return;
    }

    let nextProfiles = profiles;

    const model = pendingModelRef.current;
    if (model) {
      pendingModelRef.current = null;
      const current = getRecentModelEntries(nextProfiles, agent);
      const nextEntries = touchRecentModel(current, model);
      nextProfiles = updateRecentModelEntries(nextProfiles, agent, nextEntries);
    }

    const reasoningModel =
      model ??
      (selectedModelId && config
        ? getSelectedModel(config.models, selectedProviderId, selectedModelId)
        : null);
    const pendingReasoning = pendingReasoningRef.current;
    if (pendingReasoning && reasoningModel) {
      nextProfiles = setRecentReasoning(
        nextProfiles,
        agent,
        reasoningModel,
        pendingReasoning.reasoningId
      );
      pendingReasoningRef.current = null;
    }

    if (nextProfiles !== profiles) {
      setProfiles(nextProfiles);
      void profilesApi
        .updateRecent({
          executor: agent,
          recently_used_models:
            nextProfiles[agent]?.recently_used_models ?? null,
        })
        .catch((error) => {
          console.error('Failed to save recent models', error);
          void reloadSystem();
        });
    }
  }, [
    agent,
    config,
    profiles,
    reloadSystem,
    selectedModelId,
    selectedProviderId,
    setProfiles,
  ]);

  const handleModelSelect = (modelId: string | null, providerId?: string) => {
    const modelOverride = config
      ? buildModelSelectionOverride(config.models, modelId, providerId)
      : {
          model_id: modelId
            ? providerId
              ? `${providerId}/${modelId}`
              : modelId
            : null,
        };
    onOverrideChange({
      ...modelOverride,
      model_id:
        modelOverride.model_id === selection.baseModel
          ? null
          : modelOverride.model_id,
      // Removing an old redundant model override does not change the model.
      // Keep an independent explicit effort in that case.
      ...((modelOverride.model_id ?? selection.baseModel) === selection.model
        ? { reasoning_id: executorConfig?.reasoning_id ?? null }
        : {}),
    });

    pendingModelRef.current =
      modelId && config
        ? findModelForSelection(config.models, modelId, providerId)
        : null;
    pendingReasoningRef.current = null;
  };

  const handleReasoningSelect = (reasoningId: string | null) => {
    onOverrideChange({
      reasoning_id:
        reasoningId === selection.baseReasoning ? null : reasoningId,
    });
    pendingReasoningRef.current = { reasoningId };
  };

  const handleAgentSelect = (id: string | null) => {
    onOverrideChange({ agent_id: id });
  };

  const handlePermissionPolicyChange = (policy: PermissionPolicy) => {
    if (!supportsPermissions) return;
    onOverrideChange({
      permission_policy: policy === basePermissionPolicy ? null : policy,
    });
  };

  const scrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setSearchQuery('');
  }, [selectedProviderId]);

  useEffect(() => {
    if (!isOpen) {
      setSearchQuery('');
      return;
    }
    requestAnimationFrame(() => {
      const node = scrollRef.current;
      if (!node) return;
      if (selectedModelId && config) {
        const selected = getSelectedModel(
          config.models,
          selectedProviderId,
          selectedModelId
        );
        if (selected) {
          const key = getModelKey(selected);
          const selector = `[data-model-key="${escapeAttributeValue(key)}"]`;
          const target = node.querySelector(selector);
          if (target instanceof HTMLElement) {
            target.scrollIntoView({ block: 'nearest' });
            return;
          }
        }
      }
      if (!selectedModelId) {
        node.scrollTop = node.scrollHeight;
      }
    });
  }, [config, isOpen, selectedModelId, selectedProviderId]);

  const handleOpenChange = (open: boolean) => {
    setIsOpen(open);
    if (open) {
      const selected =
        selectedModelId && config
          ? getSelectedModel(config.models, selectedProviderId, selectedModelId)
          : null;
      setExpandedProviderId(selected?.provider_id ?? selectedProviderId ?? '');
    } else {
      persistPendingSelections();
    }
  };

  useEffect(() => {
    if (isOpen) return;
    persistPendingSelections();
  }, [isOpen, persistPendingSelections]);

  const presetLabel =
    resolvedPreset && resolvedPreset !== 'DEFAULT'
      ? toPrettyCase(resolvedPreset)
      : defaultLabel;

  if (!config) {
    return (
      <>
        <DropdownMenu>
          <DropdownMenuTriggerButton
            size="sm"
            label={streamError ? t('modelSelector.unavailable') : loadingLabel}
            disabled
          />
          {streamError && (
            <span role="alert" className="text-sm text-error">
              {t('modelSelector.loadError')} {streamError}
            </span>
          )}
        </DropdownMenu>
      </>
    );
  }

  const showDefaultOption = selection.modelOverridden;
  const displaySelectedModel = getSelectedModel(
    config.models,
    selectedProviderId,
    selectedModelId
  );
  const reasoningLabel =
    getReasoningLabel(
      displaySelectedModel?.reasoning_options ?? EMPTY_REASONING_OPTIONS,
      selectedReasoningId
    ) ?? (selectedModelLoading ? loadingLabel : null);
  const modelLabelBase = loadingModels
    ? loadingLabel
    : (displaySelectedModel?.name ??
      selectedModelId ??
      t('modelSelector.unverified'));
  const sourceLabels: Record<string, string> = {
    override: t('modelSelector.source.override'),
    profile: t('modelSelector.source.profile'),
    configured: t('modelSelector.source.configured'),
    provider: t('modelSelector.source.provider'),
    resume: t('modelSelector.source.resume'),
    unverified: t('modelSelector.source.unverified'),
  };
  const sourceLabel = (overridden: boolean, source: string) =>
    sourceLabels[overridden ? 'override' : source] ?? sourceLabels.unverified;
  const modelTitle = sourceLabel(
    selection.modelOverridden,
    selection.modelSource
  );
  const reasoningTitle = sourceLabel(
    selection.reasoningOverridden,
    selection.reasoningSource
  );
  const modelLabel = (
    <span className="inline-flex gap-1">
      <span
        title={modelTitle}
        aria-label={`${modelLabelBase}: ${modelTitle}`}
        className={selection.modelOverridden ? 'text-brand' : 'text-normal'}
      >
        {modelLabelBase}
      </span>
      {!selection.reasoningUnsupported && (
        <>
          <span aria-hidden>·</span>
          <span
            title={reasoningTitle}
            aria-label={`${reasoningLabel ?? t(selection.reasoningUnsupported ? 'modelSelector.unsupported' : 'modelSelector.unverified')}: ${reasoningTitle}`}
            className={
              selection.reasoningOverridden ? 'text-brand' : 'text-normal'
            }
          >
            {reasoningLabel ??
              t(
                selection.reasoningUnsupported
                  ? 'modelSelector.unsupported'
                  : 'modelSelector.unverified'
              )}
          </span>
        </>
      )}
    </span>
  );

  const agentLabel = selectedAgentId
    ? (config.agents.find((entry) => entry.id === selectedAgentId)?.label ??
      toPrettyCase(selectedAgentId))
    : defaultLabel;

  const permissionMeta = permissionPolicy
    ? (permissionMetaByPolicy[permissionPolicy] ?? null)
    : null;
  const permissionSource = presetOptions?.permission_policy
    ? 'profile'
    : nativeApproval
      ? 'configured'
      : config.run_defaults?.model_source === 'resume'
        ? 'resume'
        : 'unverified';
  const permissionTitle = sourceLabel(permissionOverridden, permissionSource);
  const permissionIcon = permissionMeta?.icon ?? HandIcon;
  const isCodex = agent === BaseCodingAgent.CODEX;
  const savedExecutionMode =
    executorConfig?.execution_mode ??
    presetOptions?.execution_mode ??
    (permissionPolicy === PermissionPolicy.PLAN
      ? ExecutionMode.plan
      : ExecutionMode.code);
  const executionMode =
    savedExecutionMode === ExecutionMode.plan_with_goal
      ? ExecutionMode.plan
      : savedExecutionMode;
  const isGoalMode = executionMode === ExecutionMode.goal;
  const parallelAgentLimit = executorConfig?.goal_max_concurrent_agents ?? 0;
  const executionModeOptions = [
    { value: ExecutionMode.code, label: t('modelSelector.modeCode') },
    { value: ExecutionMode.review, label: t('modelSelector.modeReview') },
    { value: ExecutionMode.plan, label: t('modelSelector.modePlan') },
    { value: ExecutionMode.goal, label: t('modelSelector.modeGoal') },
  ];
  const handleExecutionModeChange = (nextMode: ExecutionMode) => {
    const usesGoalOptions = nextMode === ExecutionMode.goal;
    onOverrideChange({
      execution_mode: nextMode,
      ...(permissionPolicy === PermissionPolicy.PLAN
        ? { permission_policy: PermissionPolicy.SUPERVISED }
        : {}),
      ...(!usesGoalOptions
        ? {
            goal_token_budget: null,
            goal_max_concurrent_agents: null,
          }
        : {}),
    });
  };
  const parsePositiveInteger = (value: string): number | null => {
    if (!value.trim()) return null;
    const parsed = Number(value);
    return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
  };

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTriggerButton
          size="sm"
          icon={SlidersHorizontalIcon}
          aria-label={t('modelSelector.preset')}
          label={
            resolvedPreset?.toLowerCase() !== 'default'
              ? presetLabel
              : undefined
          }
          showCaret={false}
        />
        <DropdownMenuContent align="start">
          <DropdownMenuLabel>{t('modelSelector.preset')}</DropdownMenuLabel>
          {presets.length > 0 ? (
            presets.map((preset) => (
              <DropdownMenuItem
                key={preset}
                icon={preset === resolvedPreset ? CheckIcon : undefined}
                onClick={() => onPresetSelect?.(preset)}
              >
                {preset === 'DEFAULT' ? defaultLabel : toPrettyCase(preset)}
              </DropdownMenuItem>
            ))
          ) : (
            <DropdownMenuItem disabled>{presetLabel}</DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem icon={GearIcon} onClick={onAdvancedSettings}>
            {t('modelSelector.custom')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <ModelSelectorPopover
        isOpen={isOpen}
        onOpenChange={handleOpenChange}
        trigger={
          <DropdownMenuTriggerButton
            size="sm"
            label={modelLabel}
            disabled={loadingModels}
          />
        }
        config={config}
        error={streamError}
        selectedProviderId={selectedProviderId}
        selectedModelId={selectedModelId}
        selectedReasoningId={selectedReasoningId}
        reasoningOverridden={selection.reasoningOverridden}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        onModelSelect={handleModelSelect}
        onReasoningSelect={handleReasoningSelect}
        recentModelEntries={recentModelEntries}
        showDefaultOption={showDefaultOption}
        onSelectDefault={() => handleModelSelect(null)}
        scrollRef={scrollRef}
        expandedProviderId={expandedProviderId}
        onExpandedProviderIdChange={setExpandedProviderId}
        resolvedTheme={resolvedTheme}
      />

      {streamError && (
        <span role="alert" className="text-sm text-error">
          {t('modelSelector.loadError')} {streamError}
        </span>
      )}
      {!loadingModels && !streamError && config.models.length === 0 && (
        <span role="status" className="text-sm text-low">
          {t('modelSelector.noModels')}
        </span>
      )}

      {isCodex && (
        <DropdownMenu>
          <DropdownMenuTriggerButton
            title={
              executionMode === ExecutionMode.review
                ? t('modelSelector.reviewHint')
                : undefined
            }
            size="sm"
            icon={
              isGoalMode
                ? TargetIcon
                : executionMode === ExecutionMode.review
                  ? MagnifyingGlassIcon
                  : executionMode === ExecutionMode.plan
                    ? ListBulletsIcon
                    : FastForwardIcon
            }
            label={
              executionModeOptions.find(({ value }) => value === executionMode)
                ?.label ?? t('modelSelector.modeCode')
            }
          />
          <DropdownMenuContent align="start">
            <DropdownMenuLabel>
              {t('modelSelector.executionMode')}
            </DropdownMenuLabel>
            {executionModeOptions.map(({ value, label }) => (
              <DropdownMenuItem
                key={value}
                icon={executionMode === value ? CheckIcon : undefined}
                onClick={() => handleExecutionModeChange(value)}
              >
                {label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      {isCodex && isGoalMode && (
        <DropdownMenu>
          <DropdownMenuTriggerButton
            size="sm"
            icon={TargetIcon}
            label={t('modelSelector.goalSettings')}
          />
          <DropdownMenuContent align="start" className="w-72">
            <DropdownMenuLabel>
              {t('modelSelector.goalSettings')}
            </DropdownMenuLabel>
            <div className="space-y-base px-base py-base">
              <label className="block space-y-half text-sm text-normal">
                <span>{t('modelSelector.tokenBudget')}</span>
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={executorConfig?.goal_token_budget ?? ''}
                  placeholder={t('modelSelector.auto')}
                  onChange={(event) =>
                    onOverrideChange({
                      goal_token_budget: parsePositiveInteger(
                        event.currentTarget.value
                      ),
                    })
                  }
                  className="border-border bg-primary text-normal w-full rounded-sm border px-base py-half"
                />
              </label>
              <div
                role="group"
                aria-label={t('modelSelector.parallelAgents')}
                className="block space-y-half text-sm text-normal"
              >
                <span>{t('modelSelector.parallelAgents')}</span>
                <div className="flex gap-half">
                  <button
                    type="button"
                    className="border-border hover:bg-secondary rounded-sm border px-base py-half"
                    onClick={() =>
                      onOverrideChange({ goal_max_concurrent_agents: null })
                    }
                  >
                    {t('modelSelector.auto')}
                  </button>
                  <button
                    type="button"
                    className="border-border hover:bg-secondary rounded-sm border px-base py-half"
                    onClick={() =>
                      onOverrideChange({ goal_max_concurrent_agents: 0 })
                    }
                  >
                    {t('modelSelector.off')}
                  </button>
                  <input
                    type="number"
                    min={1}
                    step={1}
                    value={parallelAgentLimit > 0 ? parallelAgentLimit : ''}
                    placeholder={t('modelSelector.maximum')}
                    aria-label={t('modelSelector.parallelAgentLimit')}
                    onChange={(event) =>
                      onOverrideChange({
                        goal_max_concurrent_agents: parsePositiveInteger(
                          event.currentTarget.value
                        ),
                      })
                    }
                    className="border-border bg-primary text-normal min-w-0 flex-1 rounded-sm border px-base py-half"
                  />
                </div>
              </div>
              <p className="text-xs text-low">
                {t('modelSelector.parallelHint')}
              </p>
            </div>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      {config.permissions.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTriggerButton
            size="sm"
            icon={permissionIcon}
            label={
              <span
                className={permissionOverridden ? 'text-brand' : 'text-normal'}
              >
                {permissionMeta?.label ??
                  nativeApproval ??
                  t('modelSelector.unverified')}
              </span>
            }
            title={permissionTitle}
            aria-label={`${t('modelSelector.permissions')}: ${permissionMeta?.label ?? nativeApproval ?? t('modelSelector.unverified')} (${permissionTitle})`}
            showCaret={false}
          />
          <DropdownMenuContent align="start">
            <DropdownMenuLabel>
              {t('modelSelector.permissions')}
            </DropdownMenuLabel>
            {permissionOverridden && (
              <DropdownMenuItem
                onClick={() => onOverrideChange({ permission_policy: null })}
              >
                {t('modelSelector.clearOverride')}
              </DropdownMenuItem>
            )}
            {config.permissions
              .filter((policy) => !isCodex || policy !== PermissionPolicy.PLAN)
              .map((policy) => {
                const meta = permissionMetaByPolicy[policy];
                return (
                  <DropdownMenuItem
                    key={policy}
                    icon={meta?.icon ?? HandIcon}
                    onClick={() => handlePermissionPolicyChange(policy)}
                  >
                    {meta?.label ?? toPrettyCase(policy)}
                  </DropdownMenuItem>
                );
              })}
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      {config.agents.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTriggerButton size="sm" label={agentLabel} />
          <DropdownMenuContent align="start">
            <DropdownMenuLabel>{t('modelSelector.agent')}</DropdownMenuLabel>
            <DropdownMenuItem
              icon={selectedAgentId === null ? CheckIcon : undefined}
              onClick={() => handleAgentSelect(null)}
            >
              {t('modelSelector.default')}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            {config.agents.map((agentOption) => (
              <DropdownMenuItem
                key={agentOption.id}
                icon={
                  agentOption.id === selectedAgentId ? CheckIcon : undefined
                }
                onClick={() => handleAgentSelect(agentOption.id)}
              >
                {agentOption.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </>
  );
}
