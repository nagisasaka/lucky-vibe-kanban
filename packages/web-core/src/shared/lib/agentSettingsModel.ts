import type {
  JsonValue,
  NativeConfigFile,
  SettingControl,
  SettingDescriptor,
  SettingOperation,
  SettingScope,
  SettingsDiff,
  SettingsPatch,
  SettingsSnapshot,
} from 'shared/types';
import { AgentSettingsProvider, SettingSection } from 'shared/types';

export type AgentSettingsSectionId =
  | 'overview'
  | SettingSection
  | 'tools'
  | 'profiles'
  | 'native_files'
  | 'effective_config';

export interface AgentSettingsSection {
  id: AgentSettingsSectionId;
  descriptors: SettingDescriptor[];
}

export type AgentSettingValidationError =
  | { code: 'maxLength' | 'minimum' | 'maximum'; limit: number }
  | { code: 'format' | 'number' | 'object' | 'json' | 'option' };

export type AgentSettingsDraftEntry = {
  action: 'unchanged' | 'set' | 'unset';
  value?: JsonValue;
  raw: string;
  error?: AgentSettingValidationError;
};

export type AgentSettingsDraft = Record<string, AgentSettingsDraftEntry>;

export const PROVIDER_LABELS: Record<AgentSettingsProvider, string> = {
  [AgentSettingsProvider.codex]: 'Codex',
  [AgentSettingsProvider.claude_code]: 'Claude Code',
};

export const PROVIDER_BY_EXECUTOR: Record<string, AgentSettingsProvider> = {
  CODEX: AgentSettingsProvider.codex,
  CLAUDE_CODE: AgentSettingsProvider.claude_code,
};

const SETTING_SECTION_ORDER: SettingSection[] = [
  SettingSection.general,
  SettingSection.permissions_sandbox,
  SettingSection.instructions,
  SettingSection.environment,
  SettingSection.provider_settings,
];

export function settingKeyId(
  descriptor: Pick<SettingDescriptor, 'key'>
): string {
  return `${descriptor.key.namespace}.${descriptor.key.name}`;
}

export function buildAgentSettingsSections(
  snapshot: SettingsSnapshot | null
): AgentSettingsSection[] {
  if (!snapshot) return [];
  const sections: AgentSettingsSection[] = [
    { id: 'overview', descriptors: [] },
  ];
  for (const section of SETTING_SECTION_ORDER) {
    const descriptors = snapshot.descriptors.filter(
      (descriptor) => descriptor.section === section
    );
    if (descriptors.length > 0) {
      sections.push({
        id: section,
        descriptors,
      });
    }
  }
  sections.push({ id: 'tools', descriptors: [] });
  if (snapshot.capabilities.profile_storage) {
    sections.push({
      id: 'profiles',
      descriptors: [],
    });
  }
  if (snapshot.native_files.length > 0) {
    sections.push({
      id: 'native_files',
      descriptors: [],
    });
  }
  if (
    snapshot.effective_settings.length > 0 ||
    snapshot.unknown_native_nodes.length > 0
  ) {
    sections.push({
      id: 'effective_config',
      descriptors: [],
    });
  }
  return sections;
}

export function valueToInput(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return '';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number' || typeof value === 'string')
    return String(value);
  if (Array.isArray(value)) return value.map(String).join('\n');
  return JSON.stringify(value, null, 2);
}

function validateText(
  descriptor: SettingDescriptor,
  value: string
): AgentSettingValidationError | undefined {
  const validation = descriptor.validation;
  if (
    validation.max_length !== null &&
    validation.max_length !== undefined &&
    value.length > validation.max_length
  ) {
    return { code: 'maxLength', limit: validation.max_length };
  }
  if (validation.pattern) {
    try {
      if (!new RegExp(validation.pattern).test(value))
        return { code: 'format' };
    } catch {
      // The Adapter owns validation if a pattern cannot be compiled locally.
    }
  }
  return undefined;
}

export function parseSettingInput(
  descriptor: SettingDescriptor,
  raw: string
): { value?: JsonValue; error?: AgentSettingValidationError } {
  switch (descriptor.control) {
    case 'text':
    case 'textarea': {
      const error = validateText(descriptor, raw);
      return error ? { error } : { value: raw };
    }
    case 'number': {
      const value = Number(raw);
      if (!raw.trim() || !Number.isFinite(value))
        return { error: { code: 'number' } };
      if (
        descriptor.validation.minimum !== null &&
        descriptor.validation.minimum !== undefined &&
        value < descriptor.validation.minimum
      )
        return {
          error: { code: 'minimum', limit: descriptor.validation.minimum },
        };
      if (
        descriptor.validation.maximum !== null &&
        descriptor.validation.maximum !== undefined &&
        value > descriptor.validation.maximum
      )
        return {
          error: { code: 'maximum', limit: descriptor.validation.maximum },
        };
      return { value };
    }
    case 'string_list':
      return {
        value: raw
          .split('\n')
          .map((item) => item.trim())
          .filter(Boolean),
      };
    case 'key_value':
    case 'json': {
      if (!raw.trim())
        return { value: descriptor.value_type === 'string_map' ? {} : null };
      try {
        const value = JSON.parse(raw) as JsonValue;
        if (
          descriptor.value_type === 'string_map' &&
          (value === null || Array.isArray(value) || typeof value !== 'object')
        )
          return { error: { code: 'object' } };
        return { value };
      } catch {
        return { error: { code: 'json' } };
      }
    }
    case 'toggle':
      return { value: raw === 'true' };
    case 'select': {
      const option = descriptor.options.find(
        (candidate) => JSON.stringify(candidate.value) === raw
      );
      return option ? { value: option.value } : { error: { code: 'option' } };
    }
    default:
      return { value: raw };
  }
}

export function createAgentSettingsDraft(
  snapshot: SettingsSnapshot
): AgentSettingsDraft {
  const effectiveByKey = new Map(
    snapshot.effective_settings.map((setting) => [
      settingKeyId(setting),
      setting,
    ])
  );
  return Object.fromEntries(
    snapshot.descriptors.map((descriptor) => {
      const value = effectiveByKey.get(
        settingKeyId(descriptor)
      )?.effective_value;
      return [
        settingKeyId(descriptor),
        { action: 'unchanged', value, raw: valueToInput(value) },
      ];
    })
  );
}

export function isAgentSettingsDraftDirty(draft: AgentSettingsDraft): boolean {
  return Object.values(draft).some((entry) => entry.action !== 'unchanged');
}

export function hasDraftErrors(draft: AgentSettingsDraft): boolean {
  return Object.values(draft).some((entry) => Boolean(entry.error));
}

export function buildAgentSettingsPatch(
  snapshot: SettingsSnapshot,
  draft: AgentSettingsDraft,
  scope: SettingScope,
  projectPath?: string
): SettingsPatch {
  const operations: SettingOperation[] = [];
  for (const descriptor of snapshot.descriptors) {
    const entry = draft[settingKeyId(descriptor)];
    if (
      !entry ||
      entry.action === 'unchanged' ||
      !descriptor.capabilities.writable ||
      !descriptor.supported_scopes.includes(scope)
    )
      continue;
    if (entry.action === 'unset') {
      operations.push({ type: 'unset', data: { key: descriptor.key, scope } });
    } else if (entry.value !== undefined) {
      operations.push({
        type: 'set',
        data: { key: descriptor.key, scope, value: entry.value },
      });
    }
  }
  const expected_file_revisions: Record<string, string> = {};
  for (const file of snapshot.native_files)
    expected_file_revisions[file.id] = file.revision ?? 'missing';
  return {
    provider: snapshot.provider,
    project_path: projectPath || null,
    expected_file_revisions,
    operations,
  };
}

export function nativeFileRevision(file: NativeConfigFile): string {
  return file.revision ?? 'missing';
}

export function hasChangedFiles(diff: SettingsDiff | null): boolean {
  return Boolean(diff?.files.some((file) => file.changed));
}

export function controlInputKind(
  control: SettingControl
): 'text' | 'textarea' | 'number' | 'toggle' | 'select' | 'json' {
  if (
    control === 'key_value' ||
    control === 'json' ||
    control === 'string_list'
  )
    return 'json';
  return control;
}
