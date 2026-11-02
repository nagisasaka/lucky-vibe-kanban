import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
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
  type AgentSettingsUiError,
  agentSettingValidationMessage,
} from '@/shared/lib/agentSettingsTranslations';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowClockwiseIcon,
  CheckCircleIcon,
  CodeIcon,
  CopyIcon,
  FloppyDiskIcon,
  PencilSimpleIcon,
  PlusIcon,
  SpinnerIcon,
  TrashIcon,
  WarningCircleIcon,
} from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Switch } from '@vibe/ui/components/Switch';
import {
  AgentSettingsProvider,
  BaseCodingAgent,
  SettingScope,
} from 'shared/types';
import type {
  ConfigProfileView as ConfigProfile,
  JsonValue,
  NativeConfigFile,
  ProfileApplyRequest as ProfileApplyPreviewRequest,
  ProfileCopyPreviewView as ProfileCopyPreview,
  SettingDescriptor,
  SettingsDiff,
  SettingsPatch,
  SettingsSnapshotView as SettingsSnapshot,
} from 'shared/types';
import {
  buildAgentSettingsPatch,
  buildAgentSettingsSections,
  createAgentSettingsDraft,
  hasChangedFiles,
  hasDraftErrors,
  isAgentSettingsDraftDirty,
  nativeFileRevision,
  parseSettingInput,
  PROVIDER_BY_EXECUTOR,
  PROVIDER_LABELS,
  settingKeyId,
  type AgentSettingsDraft,
  type AgentSettingValidationError,
} from '@/shared/lib/agentSettingsModel';
import { cn } from '@/shared/lib/utils';
import { AgentToolsSettingsSection } from './AgentToolsSettingsSection';
import {
  SettingsCard,
  SettingsField,
  SettingsInput,
  SettingsSaveBar,
  SettingsTextarea,
} from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';
import {
  useSettingsHost,
  useSettingsMachineClient,
} from './SettingsHostContext';

type PendingAction =
  | { kind: 'settings'; patch: SettingsPatch }
  | {
      kind: 'native';
      patch: {
        provider: AgentSettingsProvider;
        project_path?: string | null;
        file_id: string;
        expected_revision: string;
        content: string;
      };
    }
  | { kind: 'profile'; preview: ProfileApplyPreviewRequest };

function executorForProvider(provider: AgentSettingsProvider): BaseCodingAgent {
  switch (provider) {
    case AgentSettingsProvider.claude_code:
      return BaseCodingAgent.CLAUDE_CODE;
    default:
      return BaseCodingAgent.CODEX;
  }
}

function displayJson(
  t: TFunction<'settings'>,
  value: JsonValue | undefined
): string {
  if (value === undefined) return t('agentConfiguration.ui.inherited');
  return JSON.stringify(value, null, 2);
}

function sourceLabel(
  t: TFunction<'settings'>,
  snapshot: SettingsSnapshot,
  descriptor: SettingDescriptor,
  entry: AgentSettingsDraft[string] | undefined
): string {
  if (entry?.action === 'set') return t('agentConfiguration.ui.modified');
  if (entry?.action === 'unset') return t('agentConfiguration.ui.inherit');
  const setting = snapshot.effective_settings.find(
    (candidate) => settingKeyId(candidate) === settingKeyId(descriptor)
  );
  return agentSettingsSourceLabel(t, setting?.effective_source);
}

function parseDraftChange(
  descriptor: SettingDescriptor,
  raw: string,
  value?: JsonValue
): { value?: JsonValue; error?: AgentSettingValidationError } {
  if (descriptor.control === 'select') {
    return parseSettingInput(descriptor, raw);
  }
  if (descriptor.control === 'toggle') return { value };
  return parseSettingInput(descriptor, raw);
}

export function AgentConfigurationSettingsPanel(props: {
  executor: BaseCodingAgent;
  variant: string | null;
}) {
  const { selectedHostId } = useSettingsHost();
  return (
    <AgentConfigurationSettingsContent
      key={`${selectedHostId}:${props.executor}:${props.variant}`}
      {...props}
    />
  );
}

function AgentConfigurationSettingsContent({
  executor,
  variant,
}: {
  executor: BaseCodingAgent;
  variant: string | null;
}) {
  const { t, i18n } = useTranslation('settings');
  const sectionLabels = agentSettingsSectionLabels(t);
  const errorMessages = agentSettingsErrorMessages(t);
  const stateLabels = agentSettingsStateLabels(t);
  const machineClient = useSettingsMachineClient();
  const { canEdit } = useSettingsHost();
  const { setDirty: setContextDirty } = useSettingsDirty();
  const provider =
    PROVIDER_BY_EXECUTOR[executor] ?? AgentSettingsProvider.codex;
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [draft, setDraft] = useState<AgentSettingsDraft>({});
  const [profiles, setProfiles] = useState<ConfigProfile[]>([]);
  const [activeSection, setActiveSection] = useState<string>('overview');
  const [scope, setScope] = useState<SettingScope>(SettingScope.user);
  const [projectPath, setProjectPath] = useState('');
  const [loadedProjectPath, setLoadedProjectPath] = useState<string | null>(
    null
  );
  const [loadFailed, setLoadFailed] = useState(false);
  const [loading, setLoading] = useState(false);
  const [profilesLoading, setProfilesLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AgentSettingsUiError | null>(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  const noticeMessages = {
    noChanges: t('agentConfiguration.ui.noChanges'),
    applied: t('agentConfiguration.ui.applied'),
    profileSaved: t('agentConfiguration.ui.profileSaved'),
    duplicated: t('agentConfiguration.ui.duplicated'),
    renamed: t('agentConfiguration.ui.renamed'),
    deleted: t('agentConfiguration.ui.deleted'),
    copied: t('agentConfiguration.ui.copied'),
  };
  const [notice, setNotice] = useState<keyof typeof noticeMessages | null>(
    null
  );
  const [diff, setDiff] = useState<SettingsDiff | null>(null);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [copyPreview, setCopyPreview] = useState<ProfileCopyPreview | null>(
    null
  );
  const [rawFile, setRawFile] = useState<NativeConfigFile | null>(null);
  const [rawContent, setRawContent] = useState('');
  const [rawOriginal, setRawOriginal] = useState('');
  const requestSequence = useRef(0);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      requestSequence.current += 1;
    };
  }, []);
  const writeBlocked =
    !canEdit ||
    loading ||
    loadFailed ||
    !snapshot ||
    loadedProjectPath !== projectPath.trim();

  const loadSettings = useCallback(
    async (nextProjectPath?: string) => {
      if (!machineClient) return;
      const sequence = ++requestSequence.current;
      setLoading(true);
      setError(null);
      setErrorDetail(null);
      try {
        const inventory = await machineClient.discoverAgentSettings({
          provider,
          project_path: nextProjectPath || null,
        });
        if (sequence !== requestSequence.current) return;
        const next =
          inventory.providers.find(
            (candidate) => candidate.provider === provider
          ) ?? null;
        setSnapshot(next);
        setLoadedProjectPath(nextProjectPath?.trim() ?? '');
        setLoadFailed(inventory.errors.length > 0);
        setDraft(next ? createAgentSettingsDraft(next) : {});
        setDiff(null);
        setPending(null);
        setRawFile(null);
        if (inventory.errors.length > 0 && !next) {
          setError('discovery');
        }
      } catch (nextError) {
        if (sequence === requestSequence.current) {
          setErrorDetail(errorDiagnostic(nextError));
          setError(agentSettingsErrorCode(nextError));
          setLoadFailed(true);
        }
      } finally {
        if (sequence === requestSequence.current) setLoading(false);
      }
    },
    [machineClient, provider]
  );

  const loadProfiles = useCallback(async () => {
    if (!machineClient) return;
    setProfilesLoading(true);
    try {
      setProfiles(await machineClient.listAgentSettingsProfiles({ provider }));
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setProfilesLoading(false);
    }
  }, [machineClient, provider]);

  useEffect(() => {
    setSnapshot(null);
    setDraft({});
    setActiveSection('overview');
    void loadSettings();
    void loadProfiles();
  }, [loadProfiles, loadSettings]);

  const sections = useMemo(
    () => buildAgentSettingsSections(snapshot),
    [snapshot]
  );

  useEffect(() => {
    if (
      sections.length > 0 &&
      !sections.some((section) => section.id === activeSection)
    ) {
      setActiveSection(sections[0].id);
    }
  }, [activeSection, sections]);

  useEffect(() => {
    if (!projectPath.trim() && scope === SettingScope.project) {
      setScope(SettingScope.user);
    }
  }, [projectPath, scope]);

  const projectScopeSupported = Boolean(
    snapshot?.descriptors.some((descriptor) =>
      descriptor.supported_scopes.includes(SettingScope.project)
    )
  );

  useEffect(() => {
    if (!projectScopeSupported && scope === SettingScope.project) {
      setScope(SettingScope.user);
    }
  }, [projectScopeSupported, scope]);

  const draftDirty = isAgentSettingsDraftDirty(draft);
  const rawDirty = rawFile !== null && rawContent !== rawOriginal;
  const isDirty = draftDirty || rawDirty;

  useEffect(() => {
    setContextDirty(`agent-settings:${provider}`, isDirty);
    return () => setContextDirty(`agent-settings:${provider}`, false);
  }, [isDirty, provider, setContextDirty]);

  const applySnapshot = (next: SettingsSnapshot) => {
    setSnapshot(next);
    setDraft(createAgentSettingsDraft(next));
    setDiff(null);
    setPending(null);
    setRawFile(null);
    setRawContent('');
    setRawOriginal('');
  };

  const patch = useMemo(
    () =>
      snapshot
        ? buildAgentSettingsPatch(
            snapshot,
            draft,
            scope,
            projectPath.trim() || undefined
          )
        : null,
    [draft, projectPath, scope, snapshot]
  );

  const previewSettings = async () => {
    if (!machineClient || !patch || !snapshot || writeBlocked) return;
    if (hasDraftErrors(draft)) {
      setError('invalidValues');
      return;
    }
    if (patch.operations.length === 0) {
      setNotice('noChanges');
      return;
    }
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    setNotice(null);
    try {
      const nextDiff = await machineClient.diffAgentSettings(patch);
      setDiff(nextDiff);
      setPending({ kind: 'settings', patch });
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const confirmPending = async () => {
    if (!machineClient || !pending || writeBlocked) return;
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      let nextSnapshot: SettingsSnapshot;
      if (pending.kind === 'settings') {
        nextSnapshot = await machineClient.applyAgentSettings({
          patch: pending.patch,
          confirmed: true,
        });
      } else if (pending.kind === 'native') {
        nextSnapshot = await machineClient.applyAgentSettingsNativeFile({
          patch: pending.patch,
          confirmed: true,
        });
      } else {
        nextSnapshot = await machineClient.applyAgentSettingsProfile({
          preview: pending.preview,
          confirmed: true,
        });
      }
      applySnapshot(nextSnapshot);
      await loadProfiles();
      setNotice('applied');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const discardDraft = () => {
    if (snapshot) setDraft(createAgentSettingsDraft(snapshot));
    if (rawFile) {
      setRawContent(rawOriginal);
    }
    setDiff(null);
    setPending(null);
    setNotice(null);
    setError(null);
    setErrorDetail(null);
  };

  const updateDraft = (
    descriptor: SettingDescriptor,
    raw: string,
    value?: JsonValue,
    parseError?: AgentSettingValidationError
  ) => {
    const id = settingKeyId(descriptor);
    setDraft((previous) => ({
      ...previous,
      [id]: {
        ...previous[id],
        action: 'set',
        raw,
        value,
        error: parseError,
      },
    }));
    setDiff(null);
    setPending(null);
  };

  const resetDraft = (descriptor: SettingDescriptor) => {
    const id = settingKeyId(descriptor);
    setDraft((previous) => ({
      ...previous,
      [id]: {
        ...previous[id],
        action: 'unset',
        raw: '',
        value: undefined,
        error: undefined,
      },
    }));
    setDiff(null);
    setPending(null);
  };

  const previewNativeFile = async () => {
    if (!machineClient || !snapshot || !rawFile || writeBlocked) return;
    const nextPatch = {
      provider,
      project_path: projectPath.trim() || null,
      file_id: rawFile.id,
      expected_revision: nativeFileRevision(rawFile),
      content: rawContent,
    };
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      const nextDiff = await machineClient.diffAgentSettingsNativeFile({
        patch: nextPatch,
        confirmed_sensitive_read: true,
      });
      setDiff(nextDiff);
      setPending({ kind: 'native', patch: nextPatch });
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const openNativeFile = async (file: NativeConfigFile) => {
    if (
      !machineClient ||
      busy ||
      loading ||
      !canEdit ||
      loadedProjectPath !== projectPath.trim()
    )
      return;
    if (!window.confirm(t('agentConfiguration.ui.readConfirm'))) return;
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      const read = await machineClient.readAgentSettingsNativeFile({
        provider,
        project_path: projectPath.trim() || null,
        file_id: file.id,
        expected_revision: nativeFileRevision(file),
        confirmed_sensitive_read: true,
      });
      if (!mounted.current) return;
      setRawFile(read);
      setRawOriginal(read.raw_content ?? '');
      setRawContent(read.raw_content ?? '');
      setDiff(null);
      setPending(null);
    } catch (nextError) {
      if (mounted.current) setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  const saveProfile = async () => {
    if (!machineClient || !snapshot || writeBlocked) return;
    const name = window.prompt(t('agentConfiguration.ui.profileName'));
    if (!name?.trim()) return;
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      await machineClient.saveAgentSettingsProfile({
        type: 'capture',
        data: {
          patch: buildAgentSettingsPatch(
            snapshot,
            draft,
            scope,
            projectPath.trim() || undefined
          ),
          executor_profile: {
            executor,
            variant: variant === 'DEFAULT' ? null : variant,
          },
          name: name.trim(),
        },
      });
      await loadProfiles();
      setNotice('profileSaved');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const duplicateProfile = async (profile: ConfigProfile) => {
    if (!machineClient) return;
    const name = window.prompt(
      t('agentConfiguration.ui.newProfileName'),
      t('agentConfiguration.ui.duplicateName', { name: profile.name })
    );
    if (!name?.trim()) return;
    setBusy(true);
    try {
      await machineClient.saveAgentSettingsProfile({
        type: 'duplicate',
        data: {
          source: { id: profile.id, expected_revision: profile.revision },
          name: name.trim(),
        },
      });
      await loadProfiles();
      setNotice('duplicated');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const renameProfile = async (profile: ConfigProfile) => {
    if (!machineClient) return;
    const name = window.prompt(
      t('agentConfiguration.ui.profileName'),
      profile.name
    );
    if (!name?.trim() || name.trim() === profile.name) return;
    setBusy(true);
    try {
      await machineClient.saveAgentSettingsProfile({
        type: 'rename',
        data: {
          source: { id: profile.id, expected_revision: profile.revision },
          name: name.trim(),
        },
      });
      await loadProfiles();
      setNotice('renamed');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const deleteProfile = async (profile: ConfigProfile) => {
    if (
      !machineClient ||
      !window.confirm(
        t('agentConfiguration.ui.deleteConfirm', { name: profile.name })
      )
    )
      return;
    setBusy(true);
    try {
      await machineClient.saveAgentSettingsProfile({
        type: 'delete',
        data: {
          source: { id: profile.id, expected_revision: profile.revision },
        },
      });
      await loadProfiles();
      setNotice('deleted');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const previewProfileApply = async (profile: ConfigProfile) => {
    if (!machineClient || !snapshot || writeBlocked) return;
    const expected_file_revisions = Object.fromEntries(
      snapshot.native_files.map((file) => [file.id, nativeFileRevision(file)])
    );
    const request: ProfileApplyPreviewRequest = {
      reference: { id: profile.id, expected_revision: profile.revision },
      project_path: projectPath.trim() || null,
      scope,
      expected_file_revisions,
    };
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      const nextDiff =
        await machineClient.previewAgentSettingsProfileApply(request);
      setDiff(nextDiff);
      setPending({ kind: 'profile', preview: request });
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const previewProfileCopy = async (profile: ConfigProfile) => {
    if (!machineClient) return;
    const target = window.prompt(
      t('agentConfiguration.ui.targetProvider'),
      'codex'
    );
    if (
      !target ||
      !Object.prototype.hasOwnProperty.call(PROVIDER_LABELS, target)
    )
      return;
    const targetName = window.prompt(
      t('agentConfiguration.ui.copiedName'),
      `${profile.name} (${PROVIDER_LABELS[target as AgentSettingsProvider]})`
    );
    if (!targetName?.trim()) return;
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      const preview = await machineClient.previewAgentSettingsProfileCopy({
        source: { id: profile.id, expected_revision: profile.revision },
        target_provider: target as AgentSettingsProvider,
        target_executor_profile: {
          executor: executorForProvider(target as AgentSettingsProvider),
          variant: null,
        },
        target_name: targetName.trim(),
      });
      setCopyPreview(preview);
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const saveCopiedProfile = async () => {
    if (!machineClient || !copyPreview) return;
    setBusy(true);
    try {
      await machineClient.saveAgentSettingsProfile({
        type: 'copy',
        data: { request: copyPreview.request },
      });
      setCopyPreview(null);
      await loadProfiles();
      setNotice('copied');
    } catch (nextError) {
      setErrorDetail(errorDiagnostic(nextError));
      setError(agentSettingsErrorCode(nextError));
    } finally {
      setBusy(false);
    }
  };

  const renderOverview = () => (
    <SettingsCard
      title={t('agentConfiguration.ui.providerTitle', {
        provider: PROVIDER_LABELS[provider],
      })}
      description={t('agentConfiguration.ui.nativeAuthority')}
      headerAction={
        <PrimaryButton
          variant="tertiary"
          value={t('agentConfiguration.ui.refresh')}
          onClick={() => void loadSettings(projectPath.trim() || undefined)}
          disabled={loading || busy || !machineClient}
          actionIcon={loading ? 'spinner' : undefined}
        />
      }
    >
      <div className="grid gap-3 md:grid-cols-3">
        <InfoTile
          label={t('agentConfiguration.ui.installed')}
          value={
            snapshot?.installed
              ? t('agentConfiguration.ui.yes')
              : t('agentConfiguration.ui.notDetected')
          }
        />
        <InfoTile
          label={t('agentConfiguration.ui.version')}
          value={
            snapshot?.provider_version ?? t('agentConfiguration.ui.unknown')
          }
        />
        <InfoTile
          label={t('agentConfiguration.ui.executable')}
          value={
            snapshot?.executable_path ?? t('agentConfiguration.ui.notFound')
          }
          mono
        />
      </div>
      <SettingsField
        label={t('agentConfiguration.ui.projectPath')}
        description={t('agentConfiguration.ui.projectHelp')}
      >
        <div className="flex gap-2">
          <SettingsInput
            value={projectPath}
            onChange={(value) => {
              setProjectPath(value);
              setPending(null);
              setDiff(null);
              setRawFile(null);
              setRawContent('');
              setRawOriginal('');
              setCopyPreview(null);
            }}
            placeholder="/absolute/path/to/project"
            disabled={busy || loading}
          />
          <button
            type="button"
            className="rounded-sm border border-border px-3 text-low hover:text-normal disabled:opacity-40"
            aria-label={t('agentConfiguration.ui.discoverProject')}
            disabled={busy || loading}
            onClick={() => void loadSettings(projectPath.trim() || undefined)}
          >
            <ArrowClockwiseIcon className="size-icon-xs" aria-hidden="true" />
          </button>
        </div>
      </SettingsField>
      <SettingsField
        label={t('agentConfiguration.ui.scope')}
        description={t('agentConfiguration.ui.scopeHelp')}
      >
        <select
          value={scope}
          onChange={(event) => {
            setScope(event.target.value as SettingScope);
            setPending(null);
            setDiff(null);
          }}
          disabled={busy}
          className="w-full rounded-sm border border-border bg-secondary px-base py-half text-sm text-high focus:outline-none focus:ring-1 focus:ring-brand"
        >
          <option value={SettingScope.user}>
            {t('agentConfiguration.ui.userConfig')}
          </option>
          <option
            value={SettingScope.project}
            disabled={!projectPath.trim() || !projectScopeSupported}
          >
            {t('agentConfiguration.ui.projectConfig')}
          </option>
        </select>
      </SettingsField>
      <div
        className="flex flex-wrap gap-2"
        aria-label={t('agentConfiguration.ui.capabilities')}
      >
        {snapshot &&
          Object.entries(snapshot.capabilities).map(([key, enabled]) => (
            <span
              key={key}
              className="rounded bg-secondary px-2 py-1 text-xs text-low"
            >
              {stateLabels.capabilities[
                key as keyof typeof stateLabels.capabilities
              ] ?? key}
              :{' '}
              {enabled
                ? t('agentConfiguration.ui.available')
                : t('agentConfiguration.ui.unsupported')}
            </span>
          ))}
      </div>
      {snapshot && !snapshot.installed && (
        <div className="flex items-start gap-2 rounded-sm border border-border bg-secondary/30 p-3 text-sm text-low">
          <WarningCircleIcon
            className="mt-0.5 size-icon-sm shrink-0"
            aria-hidden="true"
          />
          {t('agentConfiguration.ui.notInstalled')}
        </div>
      )}
      {snapshot?.limitations.map((limitation) => (
        <div
          key={limitation}
          className="rounded-sm border border-border bg-secondary/20 p-3 text-xs text-low"
        >
          {agentSettingsExplanation(t, limitation)}
        </div>
      ))}
      {snapshot?.errors.map((item) => (
        <div
          key={`${item.file_id ?? ''}:${item.setting_key ?? ''}:${item.message}`}
          className="rounded-sm border border-error/40 bg-error/10 p-3 text-sm text-error"
          role="alert"
        >
          {t('agentConfiguration.errors.nativeInvalid')} {item.message}{' '}
          {item.recovery} {item.file_id} {item.setting_key}
        </div>
      ))}
    </SettingsCard>
  );

  const renderSettings = (descriptors: SettingDescriptor[]) => (
    <SettingsCard
      title={
        sectionLabels[activeSection as keyof typeof sectionLabels] ??
        t('agentConfiguration.sections.settings')
      }
      description={t('agentConfiguration.ui.fieldsHelp')}
    >
      <div className="space-y-5">
        {descriptors.map((descriptor) => {
          const id = settingKeyId(descriptor);
          const descriptorText = agentSettingDescriptorText(
            t,
            provider,
            descriptor
          );
          const entry = draft[id] ?? { action: 'unchanged' as const, raw: '' };
          const resettable = descriptor.capabilities.resettable;
          const scopeSupported = descriptor.supported_scopes.includes(scope);
          const disabled =
            !descriptor.capabilities.writable ||
            busy ||
            !scopeSupported ||
            writeBlocked;
          const selectValue =
            entry.value === undefined ? '' : JSON.stringify(entry.value);
          const update = (raw: string, nextValue?: JsonValue) => {
            const parsed = parseDraftChange(descriptor, raw, nextValue);
            updateDraft(descriptor, raw, parsed.value, parsed.error);
          };
          return (
            <SettingsField
              key={id}
              label={descriptorText.label}
              description={descriptorText.description}
              error={agentSettingValidationMessage(t, entry.error)}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-low">
                  <span className="rounded bg-secondary px-1.5 py-0.5">
                    {sourceLabel(t, snapshot!, descriptor, entry)}
                  </span>
                  <span className="ml-2">
                    {stateLabels.activation[descriptor.activation]}
                  </span>
                </span>
                {resettable && (
                  <button
                    type="button"
                    className="text-xs text-low underline hover:text-normal disabled:opacity-40"
                    disabled={disabled}
                    onClick={() => resetDraft(descriptor)}
                  >
                    {t('agentConfiguration.ui.restore')}
                  </button>
                )}
              </div>
              {!scopeSupported && (
                <div className="text-xs text-low">
                  {t('agentConfiguration.ui.unsupportedScope', {
                    scope:
                      scope === SettingScope.user
                        ? t('agentConfiguration.ui.userConfig')
                        : t('agentConfiguration.ui.projectConfig'),
                  })}
                </div>
              )}
              {snapshot?.withheld_setting_keys.includes(id) &&
                entry.action === 'unchanged' && (
                  <p className="text-xs text-low">
                    {t('agentConfiguration.ui.withheldHelp')}
                  </p>
                )}
              {descriptor.control === 'toggle' ? (
                <div className="flex items-center gap-3">
                  <Switch
                    checked={
                      entry.action === 'unset' ? false : Boolean(entry.value)
                    }
                    disabled={disabled}
                    aria-label={descriptorText.label}
                    onCheckedChange={(checked) =>
                      update(checked ? 'true' : 'false', checked)
                    }
                  />
                  <span className="text-sm text-normal">
                    {entry.value
                      ? t('agentConfiguration.ui.enabled')
                      : t('agentConfiguration.ui.disabled')}
                  </span>
                </div>
              ) : descriptor.control === 'select' ? (
                <select
                  value={selectValue}
                  disabled={disabled}
                  aria-label={descriptorText.label}
                  onChange={(event) => update(event.target.value)}
                  className="w-full rounded-sm border border-border bg-secondary px-base py-half text-sm text-high focus:outline-none focus:ring-1 focus:ring-brand"
                >
                  <option value="">
                    {t('agentConfiguration.ui.followNative')}
                  </option>
                  {descriptor.options.map((option) => (
                    <option
                      key={JSON.stringify(option.value)}
                      value={JSON.stringify(option.value)}
                    >
                      {agentSettingOptionLabel(t, provider, descriptor, option)}
                    </option>
                  ))}
                </select>
              ) : descriptor.control === 'textarea' ||
                descriptor.control === 'string_list' ||
                descriptor.control === 'key_value' ||
                descriptor.control === 'json' ? (
                <SettingsTextarea
                  value={entry.raw}
                  onChange={(raw) => update(raw)}
                  disabled={disabled}
                  rows={
                    descriptor.control === 'json' ||
                    descriptor.control === 'key_value'
                      ? 5
                      : 3
                  }
                  monospace={
                    descriptor.control === 'json' ||
                    descriptor.control === 'key_value'
                  }
                />
              ) : (
                <SettingsInput
                  value={entry.raw}
                  onChange={(raw) => update(raw)}
                  disabled={disabled}
                  error={Boolean(entry.error)}
                />
              )}
            </SettingsField>
          );
        })}
      </div>
      <SettingsSaveBar
        show={draftDirty}
        saving={busy}
        saveDisabled={
          writeBlocked ||
          hasDraftErrors(draft) ||
          !snapshot?.capabilities.native_writable
        }
        onSave={() => void previewSettings()}
        onDiscard={discardDraft}
      />
    </SettingsCard>
  );

  const renderNativeFiles = () => (
    <SettingsCard
      title={t('agentConfiguration.ui.nativeFiles')}
      description={t('agentConfiguration.ui.nativeHelp')}
    >
      <div className="space-y-3">
        {snapshot?.native_files.map((file) => (
          <NativeFileRow
            key={file.id}
            file={file}
            selected={rawFile?.id === file.id}
            onEdit={() => void openNativeFile(file)}
          />
        ))}
      </div>
      {rawFile && (
        <div className="space-y-3 rounded-sm border border-border bg-secondary/20 p-3">
          <div className="flex items-center justify-between gap-2">
            <div className="min-w-0">
              <div className="text-sm font-medium text-high">
                {rawFile.path}
              </div>
              <div className="font-mono text-xs text-low">
                {t('agentConfiguration.ui.revision', {
                  revision: nativeFileRevision(rawFile),
                })}
              </div>
            </div>
            <button
              type="button"
              className="text-xs text-low underline hover:text-normal"
              onClick={() => setRawFile(null)}
            >
              {t('agentConfiguration.ui.closeEditor')}
            </button>
          </div>
          <SettingsTextarea
            value={rawContent}
            onChange={setRawContent}
            rows={14}
            monospace
            disabled={busy}
          />
          <div className="flex justify-end gap-2">
            <PrimaryButton
              variant="tertiary"
              value={t('agentConfiguration.ui.discard')}
              onClick={() => setRawContent(rawOriginal)}
              disabled={busy || rawContent === rawOriginal}
            />
            <PrimaryButton
              value={t('agentConfiguration.ui.previewDiff')}
              onClick={() => void previewNativeFile()}
              disabled={busy || rawContent === rawOriginal}
              actionIcon={busy ? 'spinner' : undefined}
            />
          </div>
        </div>
      )}
    </SettingsCard>
  );

  const renderEffective = () => (
    <SettingsCard
      title={t('agentConfiguration.ui.effectiveConfig')}
      description={t('agentConfiguration.ui.effectiveHelp')}
    >
      <div className="overflow-x-auto rounded-sm border border-border">
        <table className="w-full text-left text-xs">
          <thead className="bg-secondary/50 text-low">
            <tr>
              <th className="px-3 py-2">
                {t('agentConfiguration.ui.setting')}
              </th>
              <th className="px-3 py-2">
                {t('agentConfiguration.ui.effective')}
              </th>
              <th className="px-3 py-2">{t('agentConfiguration.ui.source')}</th>
              <th className="px-3 py-2">
                {t('agentConfiguration.ui.observedSources')}
              </th>
            </tr>
          </thead>
          <tbody>
            {snapshot?.effective_settings.map((setting) => (
              <tr
                key={settingKeyId(setting)}
                className="border-t border-border/60 align-top"
              >
                <td className="px-3 py-2 font-mono text-normal">
                  {settingKeyId(setting)}
                </td>
                <td className="max-w-[18rem] whitespace-pre-wrap px-3 py-2 font-mono text-normal">
                  {snapshot?.withheld_setting_keys.includes(
                    settingKeyId(setting)
                  )
                    ? t('agentConfiguration.ui.withheld')
                    : displayJson(t, setting.effective_value)}
                </td>
                <td className="px-3 py-2 text-low">
                  {agentSettingsSourceLabel(t, setting.effective_source)}
                </td>
                <td className="px-3 py-2 text-low">
                  {setting.sources.length
                    ? setting.sources
                        .map(
                          (source) =>
                            `${agentSettingsSourceLabel(t, source.source)} (${source.scope === SettingScope.user ? t('agentConfiguration.ui.userConfig') : t('agentConfiguration.ui.projectConfig')})`
                        )
                        .join(', ')
                    : t('agentConfiguration.ui.noneObserved')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {snapshot?.unknown_native_nodes.length ? (
        <details className="rounded-sm border border-border bg-secondary/20 p-3">
          <summary className="cursor-pointer text-sm text-normal">
            {t('agentConfiguration.ui.unknownNodes', {
              count: snapshot.unknown_native_nodes.length,
            })}
          </summary>
          <div className="mt-3 space-y-2">
            {snapshot.unknown_native_nodes.map((node) => (
              <div
                key={`${node.file_id}:${node.native_path}`}
                className="font-mono text-xs text-low"
              >
                {node.file_id}:{node.native_path} = {JSON.stringify(node.value)}
              </div>
            ))}
          </div>
        </details>
      ) : null}
    </SettingsCard>
  );

  const renderProfiles = () => (
    <SettingsCard
      title={t('agentConfiguration.ui.profiles')}
      description={t('agentConfiguration.ui.profilesHelp')}
      headerAction={
        <PrimaryButton
          value={t('agentConfiguration.ui.saveCurrent')}
          onClick={() => void saveProfile()}
          disabled={busy || !snapshot?.capabilities.profile_storage}
          actionIcon={busy ? 'spinner' : undefined}
        />
      }
    >
      {copyPreview && (
        <div className="space-y-2 rounded-sm border border-border bg-secondary/20 p-3">
          <div className="text-sm font-medium text-high">
            {t('agentConfiguration.ui.copyPreview', {
              provider: PROVIDER_LABELS[copyPreview.request.target_provider],
            })}
          </div>
          <div className="text-xs text-low">
            {t('agentConfiguration.ui.compatible')}{' '}
            {copyPreview.compatible_keys.length
              ? copyPreview.compatible_keys.join(', ')
              : t('agentConfiguration.ui.none')}
          </div>
          <div className="text-xs text-low">
            {t('agentConfiguration.ui.skipped')}{' '}
            {copyPreview.skipped_keys.length
              ? copyPreview.skipped_keys.join(', ')
              : t('agentConfiguration.ui.none')}
          </div>
          {copyPreview.warnings.map((warning) => (
            <div key={warning} className="text-xs text-error">
              {agentSettingsExplanation(t, warning)}
            </div>
          ))}
          <div className="flex justify-end gap-2">
            <PrimaryButton
              variant="tertiary"
              value={t('common:buttons.cancel')}
              onClick={() => setCopyPreview(null)}
              disabled={busy}
            />
            <PrimaryButton
              value={t('agentConfiguration.ui.saveCopied')}
              onClick={() => void saveCopiedProfile()}
              disabled={busy}
              actionIcon={busy ? 'spinner' : undefined}
            />
          </div>
        </div>
      )}
      {profilesLoading ? (
        <div
          className="flex items-center gap-2 py-4 text-sm text-low"
          role="status"
          aria-live="polite"
        >
          <SpinnerIcon className="size-icon-xs animate-spin" />
          {t('agentConfiguration.ui.loadingProfiles')}
        </div>
      ) : profiles.length === 0 ? (
        <div className="rounded-sm border border-dashed border-border p-4 text-sm text-low">
          {t('agentConfiguration.ui.noProfiles')}
        </div>
      ) : (
        <div className="space-y-2">
          {profiles.map((profile) => (
            <div
              key={profile.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-sm border border-border bg-secondary/20 px-3 py-2"
            >
              <div className="min-w-0">
                <div className="text-sm font-medium text-normal">
                  {profile.name}
                </div>
                <div className="text-xs text-low">
                  {t('agentConfiguration.ui.profileUpdated', {
                    count: profile.setting_keys.length,
                    date: new Date(profile.updated_at).toLocaleString(
                      i18n.resolvedLanguage || i18n.language
                    ),
                  })}
                </div>
              </div>
              <div className="flex flex-wrap gap-1">
                <ToolAction
                  label={t('agentConfiguration.ui.apply')}
                  icon={CheckCircleIcon}
                  disabled={busy || !snapshot}
                  onClick={() => void previewProfileApply(profile)}
                />
                <ToolAction
                  label={t('agentConfiguration.ui.copy')}
                  icon={CopyIcon}
                  disabled={busy}
                  onClick={() => void previewProfileCopy(profile)}
                />
                <ToolAction
                  label={t('agentConfiguration.ui.duplicate')}
                  icon={PlusIcon}
                  disabled={busy}
                  onClick={() => void duplicateProfile(profile)}
                />
                <ToolAction
                  label={t('agentConfiguration.ui.rename')}
                  icon={PencilSimpleIcon}
                  disabled={busy}
                  onClick={() => void renameProfile(profile)}
                />
                <ToolAction
                  label={t('agentConfiguration.ui.delete')}
                  icon={TrashIcon}
                  danger
                  disabled={busy}
                  onClick={() => void deleteProfile(profile)}
                />
              </div>
            </div>
          ))}
        </div>
      )}
    </SettingsCard>
  );

  if (!machineClient)
    return (
      <div className="py-4 text-sm text-low">
        {t('agentConfiguration.ui.selectMachine')}
      </div>
    );
  if (loading && !snapshot)
    return (
      <div
        className="flex items-center gap-2 py-6 text-sm text-low"
        role="status"
        aria-live="polite"
      >
        <SpinnerIcon className="size-icon-sm animate-spin" />
        {t('agentConfiguration.ui.discovering', {
          provider: PROVIDER_LABELS[provider],
        })}
      </div>
    );

  const active = sections.find((section) => section.id === activeSection);
  return (
    <div
      className="mt-5 space-y-4 border-t border-border pt-5"
      data-testid="agent-settings-panel"
    >
      <div className="flex items-center gap-2">
        <CodeIcon className="size-icon-sm text-brand" aria-hidden="true" />
        <h3 className="text-base font-medium text-high">
          {t('agentConfiguration.ui.panelTitle')}
        </h3>
      </div>
      {error && (
        <div
          className="flex items-start gap-2 rounded-sm border border-error/50 bg-error/10 p-3 text-sm text-error"
          role="alert"
        >
          <WarningCircleIcon className="mt-0.5 size-icon-sm shrink-0" />
          {errorMessages[error]} {errorDetail}
        </div>
      )}
      {notice && (
        <div
          className="flex items-start gap-2 rounded-sm border border-success/50 bg-success/10 p-3 text-sm text-success"
          role="status"
        >
          <CheckCircleIcon className="mt-0.5 size-icon-sm shrink-0" />
          {noticeMessages[notice]}
        </div>
      )}
      {snapshot && writeBlocked && (
        <p role="status" className="text-sm text-warning">
          {loadedProjectPath !== projectPath.trim()
            ? t('agentConfiguration.ui.discoverFirst')
            : t('agentConfiguration.ui.cached')}
        </p>
      )}
      {!snapshot ? (
        <div className="rounded-sm border border-border p-4 text-sm text-low">
          {t('agentConfiguration.ui.noSnapshot')}
          <PrimaryButton
            value={t('agentConfiguration.ui.retry')}
            onClick={() => void loadSettings(projectPath.trim() || undefined)}
            disabled={busy || loading || !machineClient}
          />
        </div>
      ) : (
        <>
          <div
            className="flex flex-wrap gap-1 border-b border-border"
            role="tablist"
            aria-label={t('agentConfiguration.ui.sectionsLabel')}
          >
            {sections.map((section) => (
              <button
                key={section.id}
                type="button"
                role="tab"
                aria-selected={activeSection === section.id}
                onClick={() => setActiveSection(section.id)}
                className={cn(
                  'border-b-2 px-3 py-2 text-sm',
                  activeSection === section.id
                    ? 'border-brand text-brand'
                    : 'border-transparent text-low hover:text-normal'
                )}
              >
                {sectionLabels[section.id]}
              </button>
            ))}
          </div>
          {activeSection === 'overview' && renderOverview()}
          {active?.descriptors.length
            ? renderSettings(active.descriptors)
            : null}
          {activeSection === 'tools' && (
            <AgentToolsSettingsSection provider={provider} />
          )}
          {activeSection === 'profiles' && renderProfiles()}
          {activeSection === 'native_files' && renderNativeFiles()}
          {activeSection === 'effective_config' && renderEffective()}
        </>
      )}
      {diff && (
        <DiffConfirmation
          diff={diff}
          pending={pending}
          busy={busy || writeBlocked}
          onCancel={() => {
            setDiff(null);
            setPending(null);
          }}
          onConfirm={() => void confirmPending()}
        />
      )}
      <SettingsSaveBar
        show={draftDirty && active?.descriptors.length === 0}
        saving={busy}
        saveDisabled={writeBlocked}
        onSave={() => void previewSettings()}
        onDiscard={discardDraft}
      />
    </div>
  );
}

function InfoTile({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="rounded-sm border border-border bg-secondary/20 p-3">
      <div className="text-xs text-low">{label}</div>
      <div
        className={cn('mt-1 truncate text-sm text-normal', mono && 'font-mono')}
      >
        {value}
      </div>
    </div>
  );
}

function NativeFileRow({
  file,
  selected,
  onEdit,
}: {
  file: NativeConfigFile;
  selected: boolean;
  onEdit: () => void;
}) {
  const { t } = useTranslation('settings');
  const stateLabels = agentSettingsStateLabels(t);
  return (
    <div className="flex items-center justify-between gap-3 rounded-sm border border-border bg-secondary/20 px-3 py-2">
      <div className="min-w-0">
        <div className="truncate font-mono text-xs text-normal">
          {file.path}
        </div>
        <div className="text-xs text-low">
          {file.format} · {stateLabels.parse[file.parse_status]} ·{' '}
          {file.exists
            ? t('agentConfiguration.ui.revision', {
                revision: nativeFileRevision(file),
              })
            : t('agentConfiguration.ui.missing')}
        </div>
        {file.error && (
          <div className="text-xs text-error">
            {t('agentConfiguration.errors.nativeInvalid')} {file.error}
          </div>
        )}
      </div>
      <ToolAction
        label={
          selected
            ? t('agentConfiguration.ui.editing')
            : t('agentConfiguration.ui.editRaw')
        }
        icon={PencilSimpleIcon}
        disabled={!file.raw_editable || !file.exists || selected}
        onClick={onEdit}
      />
    </div>
  );
}

function DiffConfirmation({
  diff,
  pending,
  busy,
  onCancel,
  onConfirm,
}: {
  diff: SettingsDiff;
  pending: PendingAction | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t } = useTranslation('settings');
  return (
    <div
      className="space-y-3 rounded-sm border border-brand/40 bg-brand/5 p-3"
      role="dialog"
      aria-label={t('agentConfiguration.ui.diffConfirm')}
    >
      <div className="flex items-center gap-2 text-sm font-medium text-high">
        <FloppyDiskIcon className="size-icon-sm text-brand" />
        {t('agentConfiguration.ui.reviewChanges')}
      </div>
      {diff.warnings.map((warning) => (
        <div key={warning} className="text-xs text-error">
          {agentSettingsExplanation(t, warning)}
        </div>
      ))}
      {diff.files
        .filter((file) => file.changed)
        .map((file) => (
          <details
            key={file.file_id}
            open
            className="rounded-sm border border-border bg-background/40"
          >
            <summary className="cursor-pointer px-3 py-2 text-sm text-normal">
              {file.path}
            </summary>
            <div className="grid gap-2 border-t border-border p-3 md:grid-cols-2">
              <pre className="max-h-64 overflow-auto rounded bg-secondary p-2 text-xs text-low">
                {agentSettingsDiffText(
                  t,
                  file.before,
                  pending?.kind === 'native'
                )}
              </pre>
              <pre className="max-h-64 overflow-auto rounded bg-secondary p-2 text-xs text-normal">
                {agentSettingsDiffText(
                  t,
                  file.after,
                  pending?.kind === 'native'
                )}
              </pre>
            </div>
          </details>
        ))}
      {!hasChangedFiles(diff) && (
        <div className="text-sm text-low">
          {t('agentConfiguration.ui.noBytes')}
        </div>
      )}
      <div className="flex justify-end gap-2">
        <PrimaryButton
          variant="tertiary"
          value={t('common:buttons.cancel')}
          onClick={onCancel}
          disabled={busy}
        />
        <PrimaryButton
          value={
            pending?.kind === 'profile'
              ? t('agentConfiguration.ui.confirmProfile')
              : t('agentConfiguration.ui.confirmApply')
          }
          onClick={onConfirm}
          disabled={busy || !hasChangedFiles(diff)}
          actionIcon={busy ? 'spinner' : undefined}
        />
      </div>
    </div>
  );
}

function ToolAction({
  label,
  icon: Icon,
  onClick,
  disabled,
  danger,
}: {
  label: string;
  icon: typeof PlusIcon;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-sm px-2 py-1 text-xs',
        danger
          ? 'text-error hover:bg-error/10'
          : 'text-low hover:bg-secondary hover:text-normal',
        disabled && 'cursor-not-allowed opacity-40'
      )}
    >
      <Icon className="size-icon-2xs" aria-hidden="true" />
      {label}
    </button>
  );
}
