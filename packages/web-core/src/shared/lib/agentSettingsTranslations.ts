import type { TFunction } from 'i18next';
import type {
  AgentSettingsSectionId,
  AgentSettingValidationError,
} from './agentSettingsModel';

export function agentSettingsSectionLabels(
  t: TFunction<'settings'>
): Record<AgentSettingsSectionId, string> {
  return {
    overview: t('settings:agentConfiguration.sections.overview'),
    general: t('settings:agentConfiguration.sections.general'),
    permissions_sandbox: t('settings:agentConfiguration.sections.permissions'),
    instructions: t('settings:agentConfiguration.sections.instructions'),
    environment: t('settings:agentConfiguration.sections.environment'),
    provider_settings: t('settings:agentConfiguration.sections.provider'),
    tools: t('settings:agentConfiguration.sections.tools'),
    profiles: t('settings:agentConfiguration.sections.profiles'),
    native_files: t('settings:agentConfiguration.sections.nativeFiles'),
    effective_config: t('settings:agentConfiguration.sections.effectiveConfig'),
  };
}

export function agentSettingValidationMessage(
  t: TFunction<'settings'>,
  error: AgentSettingValidationError | undefined
): string | undefined {
  if (!error) return undefined;
  switch (error.code) {
    case 'maxLength':
      return t('settings:agentConfiguration.validation.maxLength', {
        limit: error.limit,
      });
    case 'minimum':
      return t('settings:agentConfiguration.validation.minimum', {
        limit: error.limit,
      });
    case 'maximum':
      return t('settings:agentConfiguration.validation.maximum', {
        limit: error.limit,
      });
    case 'format':
      return t('settings:agentConfiguration.validation.format');
    case 'number':
      return t('settings:agentConfiguration.validation.number');
    case 'object':
      return t('settings:agentConfiguration.validation.object');
    case 'json':
      return t('settings:agentConfiguration.validation.json');
    case 'option':
      return t('settings:agentConfiguration.validation.option');
  }
}

export type AgentSettingsUiError =
  | 'invalid_request'
  | 'invalid_configuration'
  | 'not_found'
  | 'stale_revision'
  | 'unsupported'
  | 'unsafe_path'
  | 'validation_failed'
  | 'io'
  | 'rollback_failed'
  | 'verification_failed'
  | 'unconfirmed'
  | 'invalidValues'
  | 'discovery';

export function agentSettingsErrorCode(error: unknown): AgentSettingsUiError {
  const detail =
    error && typeof error === 'object' && 'error_data' in error
      ? (error as { error_data?: { code?: unknown } }).error_data
      : undefined;
  switch (detail?.code) {
    case 'invalid_request':
    case 'invalid_configuration':
    case 'not_found':
    case 'stale_revision':
    case 'unsupported':
    case 'unsafe_path':
    case 'validation_failed':
    case 'io':
    case 'rollback_failed':
    case 'verification_failed':
      return detail.code;
    default:
      return 'unconfirmed';
  }
}

export function agentSettingsErrorMessages(
  t: TFunction<'settings'>
): Record<AgentSettingsUiError, string> {
  return {
    invalid_request: t('settings:agentConfiguration.errors.invalidRequest'),
    invalid_configuration: t(
      'settings:agentConfiguration.errors.invalidConfiguration'
    ),
    not_found: t('settings:agentConfiguration.errors.notFound'),
    stale_revision: t('settings:agentConfiguration.errors.staleRevision'),
    unsupported: t('settings:agentConfiguration.errors.unsupported'),
    unsafe_path: t('settings:agentConfiguration.errors.unsafePath'),
    validation_failed: t('settings:agentConfiguration.errors.validationFailed'),
    io: t('settings:agentConfiguration.errors.io'),
    rollback_failed: t('settings:agentConfiguration.errors.rollbackFailed'),
    verification_failed: t(
      'settings:agentConfiguration.errors.verificationFailed'
    ),
    unconfirmed: t('settings:agentConfiguration.errors.unconfirmed'),
    invalidValues: t('settings:agentConfiguration.ui.invalidValues'),
    discovery: t('settings:agentConfiguration.errors.discovery'),
  };
}

export function agentSettingsStateLabels(t: TFunction<'settings'>) {
  return {
    capabilities: {
      readable: t('settings:agentConfiguration.state.readable'),
      native_writable: t('settings:agentConfiguration.state.nativeWritable'),
      profile_storage: t('settings:agentConfiguration.state.profileStorage'),
      per_run_overrides: t('settings:agentConfiguration.state.perRunOverrides'),
      raw_editable: t('settings:agentConfiguration.state.rawEditable'),
    },
    activation: {
      immediate: t('settings:agentConfiguration.state.immediate'),
      next_session: t('settings:agentConfiguration.state.nextSession'),
      explicit_restart: t('settings:agentConfiguration.state.explicitRestart'),
    },
    parse: {
      missing: t('settings:agentConfiguration.state.missing'),
      parsed: t('settings:agentConfiguration.state.parsed'),
      invalid: t('settings:agentConfiguration.state.invalid'),
      unsupported: t('settings:agentConfiguration.ui.unsupported'),
    },
  };
}

export function agentSettingDescriptorText(
  t: TFunction<'settings'>,
  provider: string,
  descriptor: {
    key: { namespace: string; name: string };
    label: string;
    description: string;
  }
) {
  switch (`${provider}:${descriptor.key.namespace}.${descriptor.key.name}`) {
    case 'codex:common.model':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_common_model.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_common_model.description'
        ),
      };
    case 'codex:common.reasoning':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_common_reasoning.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_common_reasoning.description'
        ),
      };
    case 'codex:common.approval_policy':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_common_approval_policy.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_common_approval_policy.description'
        ),
      };
    case 'codex:common.sandbox_mode':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_common_sandbox_mode.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_common_sandbox_mode.description'
        ),
      };
    case 'codex:codex.web_search':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_codex_web_search.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_codex_web_search.description'
        ),
      };
    case 'codex:common.environment':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.codex_common_environment.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.codex_common_environment.description'
        ),
      };
    case 'claude_code:common.model':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_common_model.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_common_model.description'
        ),
      };
    case 'claude_code:common.reasoning':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_common_reasoning.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_common_reasoning.description'
        ),
      };
    case 'claude_code:common.permission_mode':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_common_permission_mode.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_common_permission_mode.description'
        ),
      };
    case 'claude_code:claude_code.permission_allow':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_permission_allow.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_permission_allow.description'
        ),
      };
    case 'claude_code:claude_code.permission_deny':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_permission_deny.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_permission_deny.description'
        ),
      };
    case 'claude_code:common.environment':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_common_environment.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_common_environment.description'
        ),
      };
    case 'claude_code:claude_code.output_style':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_output_style.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_output_style.description'
        ),
      };
    case 'claude_code:claude_code.enabled_plugins':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_enabled_plugins.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_enabled_plugins.description'
        ),
      };
    case 'claude_code:claude_code.hooks':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_hooks.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_hooks.description'
        ),
      };
    case 'claude_code:claude_code.status_line':
      return {
        label: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_status_line.label'
        ),
        description: t(
          'settings:agentConfiguration.descriptors.claude_code_claude_code_status_line.description'
        ),
      };
    default:
      return { label: descriptor.label, description: descriptor.description };
  }
}

export function agentSettingOptionLabel(
  t: TFunction<'settings'>,
  provider: string,
  descriptor: { key: { namespace: string; name: string } },
  option: { value: unknown; label: string }
): string {
  switch (
    `${provider}:${descriptor.key.namespace}.${descriptor.key.name}:${JSON.stringify(option.value)}`
  ) {
    case 'codex:common.reasoning:"minimal"':
      return t('settings:agentConfiguration.options.codex_reasoning_minimal');
    case 'codex:common.reasoning:"low"':
      return t('settings:agentConfiguration.options.codex_reasoning_low');
    case 'codex:common.reasoning:"medium"':
      return t('settings:agentConfiguration.options.codex_reasoning_medium');
    case 'codex:common.reasoning:"high"':
      return t('settings:agentConfiguration.options.codex_reasoning_high');
    case 'codex:common.reasoning:"xhigh"':
      return t('settings:agentConfiguration.options.codex_reasoning_xhigh');
    case 'codex:common.reasoning:"max"':
      return t('settings:agentConfiguration.options.codex_reasoning_max');
    case 'codex:common.reasoning:"ultra"':
      return t('settings:agentConfiguration.options.codex_reasoning_ultra');
    case 'codex:common.approval_policy:"untrusted"':
      return t(
        'settings:agentConfiguration.options.codex_approval_policy_untrusted'
      );
    case 'codex:common.approval_policy:"on-request"':
      return t(
        'settings:agentConfiguration.options.codex_approval_policy_on_request'
      );
    case 'codex:common.approval_policy:"never"':
      return t(
        'settings:agentConfiguration.options.codex_approval_policy_never'
      );
    case 'codex:common.sandbox_mode:"read-only"':
      return t(
        'settings:agentConfiguration.options.codex_sandbox_mode_read_only'
      );
    case 'codex:common.sandbox_mode:"workspace-write"':
      return t(
        'settings:agentConfiguration.options.codex_sandbox_mode_workspace_write'
      );
    case 'codex:common.sandbox_mode:"danger-full-access"':
      return t(
        'settings:agentConfiguration.options.codex_sandbox_mode_danger_full_access'
      );
    case 'claude_code:common.reasoning:"low"':
      return t('settings:agentConfiguration.options.claude_code_reasoning_low');
    case 'claude_code:common.reasoning:"medium"':
      return t(
        'settings:agentConfiguration.options.claude_code_reasoning_medium'
      );
    case 'claude_code:common.reasoning:"high"':
      return t(
        'settings:agentConfiguration.options.claude_code_reasoning_high'
      );
    case 'claude_code:common.permission_mode:"default"':
      return t(
        'settings:agentConfiguration.options.claude_code_permission_mode_default'
      );
    case 'claude_code:common.permission_mode:"acceptEdits"':
      return t(
        'settings:agentConfiguration.options.claude_code_permission_mode_acceptEdits'
      );
    case 'claude_code:common.permission_mode:"plan"':
      return t(
        'settings:agentConfiguration.options.claude_code_permission_mode_plan'
      );
    case 'claude_code:common.permission_mode:"dontAsk"':
      return t(
        'settings:agentConfiguration.options.claude_code_permission_mode_dontAsk'
      );
    case 'claude_code:common.permission_mode:"bypassPermissions"':
      return t(
        'settings:agentConfiguration.options.claude_code_permission_mode_bypassPermissions'
      );
    default:
      return option.label;
  }
}

export function agentSettingsSourceLabel(
  t: TFunction<'settings'>,
  source: string | null | undefined
): string {
  switch (source) {
    case null:
    case undefined:
      return t('settings:agentConfiguration.ui.default');
    case 'native_user':
      return t('settings:agentConfiguration.ui.userConfig');
    case 'native_project':
      return t('settings:agentConfiguration.ui.projectConfig');
    default:
      return source;
  }
}

// These are owned API explanations, never a transformation of native file bytes.
export function agentSettingsExplanation(
  t: TFunction<'settings'>,
  message: string
): string {
  switch (message) {
    case 'Native profiles and per-project trust entries remain available through raw TOML until their installed-version schema can be verified.':
      return t('settings:agentConfiguration.explanations.codexLimit');
    case 'Managed settings use ~/.claude/settings.json and project .claude/settings.json; ~/.claude.json authentication and MCP state is not edited here.':
      return t('settings:agentConfiguration.explanations.claudeLimit');
    case 'Provider-specific or unsupported settings were skipped; no native extension was copied.':
      return t('settings:agentConfiguration.explanations.copySkipped');
    case 'Persistent changes affect new provider sessions; active sessions are not restarted.':
      return t('settings:agentConfiguration.explanations.newSessions');
    case 'Raw editing can change provider-native fields that Vibe Kanban does not interpret.':
      return t('settings:agentConfiguration.explanations.rawEditing');
    default:
      return t('settings:agentConfiguration.explanations.original', {
        message,
      });
  }
}

export function agentSettingsDiffText(
  t: TFunction<'settings'>,
  content: string,
  nativeFile: boolean
): string {
  if (!content) return t('settings:agentConfiguration.ui.missingBytes');
  if (nativeFile) return content;
  switch (content) {
    case 'Observed native revision (values withheld)':
      return t('settings:agentConfiguration.explanations.observedRevision');
    case 'Requested settings will be applied; unrelated native values are preserved.':
      return t('settings:agentConfiguration.explanations.requestedChanges');
    case 'No change.':
      return t('settings:agentConfiguration.explanations.noChange');
    default:
      return content;
  }
}
