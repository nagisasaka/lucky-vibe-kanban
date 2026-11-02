import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowClockwiseIcon,
  CopyIcon,
  CodeIcon,
  FolderOpenIcon,
  PencilSimpleIcon,
  PlusIcon,
  SpinnerIcon,
  TrashIcon,
} from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Switch } from '@vibe/ui/components/Switch';
import type {
  AgentToolView as AgentTool,
  AgentToolDefinition,
  AgentToolWriteDefinition,
  AgentToolInventoryView as AgentToolInventory,
  AgentToolKind,
  AgentToolLocator,
  AgentToolOperationError,
  AgentToolErrorCode,
  AgentToolState,
  AgentToolProvider,
  AgentToolScope,
  McpServerWriteDefinition,
  McpTransport,
  SkillDefinition,
} from 'shared/types';
import { ApiError } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import { SettingsCard, SettingsInput } from './SettingsComponents';
import {
  useSettingsHost,
  useSettingsMachineClient,
} from './SettingsHostContext';

const PROVIDERS: AgentToolProvider[] = ['codex', 'claude_code'];

const PROVIDER_LABELS: Record<AgentToolProvider, string> = {
  codex: 'Codex',
  claude_code: 'Claude Code',
};

function encodeUtf8(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
}

function decodeUtf8(value: string): string {
  const binary = atob(value);
  return new TextDecoder().decode(
    Uint8Array.from(binary, (character) => character.charCodeAt(0))
  );
}

function locatorFor(item: AgentTool, projectPath: string): AgentToolLocator {
  return {
    provider: item.provider,
    scope: item.scope,
    kind: item.kind,
    name: item.name,
    native_path: item.native_path,
    project_path: item.scope === 'project' ? projectPath : undefined,
  };
}

type ToolError =
  | AgentToolErrorCode
  | 'unconfirmed'
  | 'noProviders'
  | 'noCopyTarget'
  | 'projectPathRequired'
  | 'invalidMcp'
  | 'invalidSkill'
  | 'unexpectedKind'
  | 'clipboard';
class ToolUiError extends Error {
  constructor(readonly code: ToolError) {
    super(code);
  }
}
function operationError(error: unknown): ToolError {
  if (error instanceof ToolUiError) return error.code;
  if (error instanceof ApiError) {
    const code = (error as ApiError<AgentToolOperationError>).error_data?.code;
    switch (code) {
      case undefined:
        return 'unconfirmed';
      case 'invalid_request':
      case 'invalid_configuration':
      case 'not_found':
      case 'collision':
      case 'stale_revision':
      case 'unsupported':
      case 'unsafe_path':
      case 'io':
      case 'verification_failed':
        return code;
    }
  }
  return 'unconfirmed';
}

function promptMcpDefinition(
  t: TFunction<'settings'>,
  transport?: McpTransport
): McpServerWriteDefinition | null {
  const action = transport ? ('preserve' as const) : ('clear' as const);
  const initial: McpServerWriteDefinition = {
    transport: transport ?? 'stdio',
    command: transport
      ? { type: action }
      : { type: 'replace', data: { value: '' } },
    args: { type: action },
    cwd: { type: action },
    url: { type: action },
    env: { type: action },
    headers: { type: action },
  };
  const value = window.prompt(
    t('agentTools.mcpPrompt'),
    JSON.stringify(initial, null, 2)
  );
  if (value === null) return null;
  return JSON.parse(value) as McpServerWriteDefinition;
}

function promptSkillDefinition(
  t: TFunction<'settings'>,
  current?: SkillDefinition
): SkillDefinition | null {
  const currentContract = current?.files.find(
    (file) => file.path === 'SKILL.md'
  );
  const initial = currentContract
    ? decodeUtf8(currentContract.content_base64)
    : '---\nname: my-skill\ndescription: Describe this Skill\n---\n\n# Instructions\n';
  const value = window.prompt(t('agentTools.skillPrompt'), initial);
  if (value === null) return null;
  const files = current?.files.filter((file) => file.path !== 'SKILL.md') ?? [];
  return {
    description: current?.description ?? null,
    files: [{ path: 'SKILL.md', content_base64: encodeUtf8(value) }, ...files],
  };
}

export function AgentToolsSettingsSection(
  props: { provider?: AgentToolProvider } = {}
) {
  const { selectedHostId } = useSettingsHost();
  return (
    <AgentToolsSettingsContent
      key={`${selectedHostId}:${props.provider ?? 'all'}`}
      {...props}
    />
  );
}

function AgentToolsSettingsContent({
  provider,
}: {
  provider?: AgentToolProvider;
} = {}) {
  const { t } = useTranslation('settings');
  const errorMessages: Record<ToolError, string> = {
    invalid_request: t('agentTools.errors.invalidRequest'),
    invalid_configuration: t('agentTools.errors.invalidConfiguration'),
    not_found: t('agentTools.errors.notFound'),
    collision: t('agentTools.errors.collision'),
    stale_revision: t('agentTools.errors.staleRevision'),
    unsupported: t('agentTools.errors.unsupported'),
    unsafe_path: t('agentTools.errors.unsafePath'),
    io: t('agentTools.errors.io'),
    verification_failed: t('agentTools.errors.verificationFailed'),
    unconfirmed: t('agentTools.errors.unconfirmed'),
    noProviders: t('agentTools.errors.noProviders'),
    noCopyTarget: t('agentTools.errors.noCopyTarget'),
    projectPathRequired: t('agentTools.errors.projectPathRequired'),
    invalidMcp: t('agentTools.errors.invalidMcp'),
    invalidSkill: t('agentTools.errors.invalidSkill'),
    unexpectedKind: t('agentTools.errors.unexpectedKind'),
    clipboard: t('agentTools.errors.clipboard'),
  };
  const toolStates: Record<AgentToolState, string> = {
    not_installed: t('agentTools.state.notInstalled'),
    enabled: t('agentTools.state.enabled'),
    disabled: t('agentTools.state.disabled'),
    error: t('agentTools.state.error'),
    unsupported: t('agentTools.state.unsupported'),
  };
  const machineClient = useSettingsMachineClient();
  const { canEdit } = useSettingsHost();
  const [inventory, setInventory] = useState<AgentToolInventory | null>(null);
  const [kind, setKind] = useState<AgentToolKind>('mcp_server');
  const [projectPath, setProjectPath] = useState('');
  const [loadedProjectPath, setLoadedProjectPath] = useState<string | null>(
    null
  );
  const [loadFailed, setLoadFailed] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<ToolError | null>(null);
  const [notice, setNotice] = useState<
    | { kind: 'path'; path: string }
    | { kind: 'copyExtensions' | 'copyWarning' }
    | null
  >(null);
  const [viewedDefinition, setViewedDefinition] = useState<{
    name: string;
    revision: string;
    definition: AgentToolDefinition;
  } | null>(null);
  const refreshSequence = useRef(0);
  const mounted = useRef(true);
  useEffect(() => {
    const refreshCounter = refreshSequence;
    mounted.current = true;
    return () => {
      mounted.current = false;
      ++refreshCounter.current;
    };
  }, []);
  const writeBlocked =
    !canEdit ||
    loading ||
    loadFailed ||
    !inventory ||
    loadedProjectPath !== projectPath.trim();
  const actionDisabled = writeBlocked || busyKey !== null;

  const loadInventory = useCallback(
    async (nextProjectPath?: string) => {
      if (!machineClient) return;
      const sequence = ++refreshSequence.current;
      setLoading(true);
      setError(null);
      try {
        const nextInventory =
          await machineClient.listAgentTools(nextProjectPath);
        if (sequence === refreshSequence.current) {
          setInventory(nextInventory);
          setLoadedProjectPath(nextProjectPath?.trim() ?? '');
          // A malformed installation in another provider must not disable an
          // independently verified provider. Its own actions are gated below.
          setLoadFailed(false);
        }
      } catch (nextError) {
        if (sequence === refreshSequence.current) {
          setError(operationError(nextError));
          setLoadFailed(true);
        }
      } finally {
        if (sequence === refreshSequence.current) {
          setLoading(false);
        }
      }
    },
    [machineClient]
  );

  const refresh = useCallback(
    () => loadInventory(projectPath.trim() || undefined),
    [loadInventory, projectPath]
  );

  useEffect(() => {
    void loadInventory();
  }, [loadInventory]);

  const inventories = useMemo(
    () =>
      (provider ? [provider] : PROVIDERS).map(
        (provider) =>
          inventory?.providers.find((entry) => entry.provider === provider) ?? {
            provider,
            installed: false,
            items: [],
            limitations: [],
            errors: [],
          }
      ),
    [inventory, provider]
  );
  const installedProviders = useMemo(
    () =>
      inventories
        .filter(
          (providerInventory) =>
            providerInventory.installed && providerInventory.errors.length === 0
        )
        .map((providerInventory) => providerInventory.provider),
    [inventories]
  );

  const run = async (key: string, operation: () => Promise<unknown>) => {
    if (actionDisabled) return;
    setBusyKey(key);
    setError(null);
    setNotice(null);
    try {
      await operation();
      if (!mounted.current) return;
      await refresh();
    } catch (nextError) {
      setError(operationError(nextError));
    } finally {
      setBusyKey(null);
    }
  };

  const handleAdd = async () => {
    if (!machineClient) return;
    if (installedProviders.length === 0) {
      setError('noProviders');
      return;
    }
    const selectedProvider =
      provider ??
      (window.prompt(
        t('agentTools.chooseProvider', {
          providers: installedProviders.join(', '),
        }),
        installedProviders[0]
      ) as AgentToolProvider | null);
    if (!selectedProvider || !installedProviders.includes(selectedProvider))
      return;
    const scope = window.prompt(
      t('agentTools.scopePrompt'),
      'user'
    ) as AgentToolScope | null;
    if (!scope || !['user', 'project'].includes(scope)) return;
    if (scope === 'project' && !projectPath.trim()) {
      setError('projectPathRequired');
      return;
    }
    const name = window.prompt(t('agentTools.namePrompt'));
    if (!name) return;
    let definition: AgentToolWriteDefinition | null;
    try {
      definition =
        kind === 'mcp_server'
          ? (() => {
              const data = promptMcpDefinition(t);
              return data ? { type: 'mcp_server', data } : null;
            })()
          : (() => {
              const data = promptSkillDefinition(t);
              return data
                ? {
                    type: 'skill',
                    data: { type: 'replace', data: { value: data } },
                  }
                : null;
            })();
    } catch (nextError) {
      setNotice(null);
      setError(
        nextError instanceof ApiError
          ? operationError(nextError)
          : kind === 'mcp_server'
            ? 'invalidMcp'
            : 'invalidSkill'
      );
      return;
    }
    if (!definition) return;
    await run(`add:${selectedProvider}:${name}`, () =>
      machineClient.createAgentTool({
        target: {
          provider: selectedProvider,
          scope,
          kind,
          name,
          project_path: scope === 'project' ? projectPath.trim() : undefined,
        },
        definition,
        replace: false,
        expected_revision: null,
      })
    );
  };

  const handleEdit = async (item: AgentTool) => {
    if (!machineClient || actionDisabled) return;
    let definition: AgentToolWriteDefinition | null;
    try {
      definition =
        item.definition.type === 'mcp_server'
          ? (() => {
              const data = promptMcpDefinition(
                t,
                item.definition.data.transport
              );
              return data ? { type: 'mcp_server', data } : null;
            })()
          : null;
      if (item.definition.type === 'skill') {
        if (!window.confirm(t('agentTools.editSensitiveConfirm'))) return;
        setBusyKey(`read:${item.provider}:${item.name}`);
        const current = await machineClient.readAgentToolDefinition({
          target: locatorFor(item, projectPath.trim()),
          expected_revision: item.revision,
          confirmed_sensitive_read: true,
        });
        if (!mounted.current) return;
        if (current.type !== 'skill') {
          setError('unexpectedKind');
          setBusyKey(null);
          return;
        }
        const data = promptSkillDefinition(t, current.data);
        const contract = data?.files.find((file) => file.path === 'SKILL.md');
        definition = contract
          ? {
              type: 'skill',
              data: {
                type: 'replace_contract',
                data: { value: decodeUtf8(contract.content_base64) },
              },
            }
          : null;
        setBusyKey(null);
      }
    } catch (nextError) {
      setNotice(null);
      setError(
        nextError instanceof ApiError
          ? operationError(nextError)
          : item.kind === 'mcp_server'
            ? 'invalidMcp'
            : 'invalidSkill'
      );
      setBusyKey(null);
      return;
    }
    if (!definition) return;
    await run(`edit:${item.provider}:${item.name}`, () =>
      machineClient.updateAgentTool({
        target: locatorFor(item, projectPath.trim()),
        expected_revision: item.revision,
        definition,
      })
    );
  };

  const handleCopy = async (item: AgentTool) => {
    if (!machineClient) return;
    const targets = installedProviders.filter(
      (provider) => provider !== item.provider
    );
    if (targets.length === 0) {
      setError('noCopyTarget');
      return;
    }
    const target = window.prompt(
      t('agentTools.chooseCopyProvider', { providers: targets.join(', ') }),
      targets[0]
    ) as AgentToolProvider | null;
    if (!target || !targets.includes(target)) return;
    const targetItem = inventory?.providers
      .find((providerInventory) => providerInventory.provider === target)
      ?.items.find(
        (candidate) =>
          candidate.scope === item.scope &&
          candidate.kind === item.kind &&
          candidate.name === item.name
      );
    if (
      targetItem &&
      !window.confirm(
        t('agentTools.replaceConfirm', {
          provider: PROVIDER_LABELS[target],
          name: item.name,
        })
      )
    ) {
      return;
    }
    await run(`copy:${item.provider}:${item.name}`, async () => {
      const result = await machineClient.copyAgentTool({
        source: locatorFor(item, projectPath.trim()),
        expected_revision: item.revision,
        target_provider: target,
        target_scope: item.scope,
        target_project_path:
          item.scope === 'project' ? projectPath.trim() : undefined,
        replace: Boolean(targetItem),
        target_expected_revision: targetItem?.revision,
      });
      if (result.warnings.length)
        setNotice({
          kind: result.warnings.every(
            (warning) =>
              warning ===
              'Provider-specific fields remain at the source and were not written to the target provider.'
          )
            ? 'copyExtensions'
            : 'copyWarning',
        });
    });
  };

  const handleReveal = async (item: AgentTool) => {
    if (!machineClient) return;
    await run(`reveal:${item.provider}:${item.name}`, async () => {
      const result = await machineClient.revealAgentTool(
        locatorFor(item, projectPath.trim())
      );
      if (!mounted.current) return;
      try {
        await navigator.clipboard.writeText(result.native_path);
      } catch {
        throw new ToolUiError('clipboard');
      }
      setNotice({ kind: 'path', path: result.native_path });
    });
  };

  const readDefinition = async (item: AgentTool) => {
    if (!machineClient || !window.confirm(t('agentTools.readSensitiveConfirm')))
      return;
    await run(`read:${item.provider}:${item.name}`, async () => {
      const definition = await machineClient.readAgentToolDefinition({
        target: locatorFor(item, projectPath.trim()),
        expected_revision: item.revision,
        confirmed_sensitive_read: true,
      });
      if (mounted.current)
        setViewedDefinition({
          name: item.name,
          revision: item.revision,
          definition,
        });
    });
  };

  return (
    <SettingsCard
      title={t('agentTools.title')}
      description={t('agentTools.description')}
      headerAction={
        <div className="flex gap-half">
          <PrimaryButton
            variant="tertiary"
            value={t('agentTools.refresh')}
            onClick={() => void refresh()}
            disabled={loading || busyKey !== null || !machineClient}
            actionIcon={loading ? 'spinner' : undefined}
          />
          <PrimaryButton
            value={t('agentTools.add')}
            onClick={() => void handleAdd()}
            disabled={
              !machineClient ||
              actionDisabled ||
              installedProviders.length === 0
            }
          />
        </div>
      }
    >
      <div className="space-y-2">
        <label
          htmlFor="agent-tools-project-path"
          className="text-sm font-medium text-normal"
        >
          {t('agentTools.projectPath')}
        </label>
        <div className="flex gap-2">
          <SettingsInput
            id="agent-tools-project-path"
            value={projectPath}
            onChange={(path) => {
              setProjectPath(path);
              setViewedDefinition(null);
            }}
            disabled={loading || busyKey !== null}
            placeholder={t('agentTools.pathPlaceholder')}
          />
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={loading || busyKey !== null || !machineClient}
            className="rounded-sm border border-border px-3 text-low hover:text-normal disabled:cursor-not-allowed disabled:opacity-40"
            title={t('agentTools.discover')}
            aria-label={t('agentTools.discover')}
          >
            <ArrowClockwiseIcon className="size-icon-xs" aria-hidden="true" />
          </button>
        </div>
      </div>

      {inventory && writeBlocked && (
        <p role="status" className="text-sm text-warning">
          {t('agentTools.blocked')}
        </p>
      )}
      {viewedDefinition && (
        <section className="space-y-2 rounded-sm border border-warning p-3">
          <p className="text-sm text-warning">
            {t('agentTools.sensitiveContents', {
              name: viewedDefinition.name,
              revision: viewedDefinition.revision.slice(0, 12),
            })}
          </p>
          <button
            type="button"
            className="underline text-sm"
            onClick={() => setViewedDefinition(null)}
          >
            {t('agentTools.hideContents')}
          </button>
          <pre
            tabIndex={0}
            className="max-h-72 overflow-auto whitespace-pre-wrap text-xs"
          >
            {JSON.stringify(viewedDefinition.definition, null, 2)}
          </pre>
        </section>
      )}

      <div className="flex border-b border-border" role="tablist">
        {(['mcp_server', 'skill'] as AgentToolKind[]).map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={kind === tab}
            aria-controls="agent-tools-provider-list"
            onClick={() => setKind(tab)}
            className={cn(
              'px-4 py-2 text-sm border-b-2 -mb-px',
              kind === tab
                ? 'border-brand text-brand'
                : 'border-transparent text-low hover:text-normal'
            )}
          >
            {tab === 'mcp_server'
              ? t('agentTools.mcp')
              : t('agentTools.skills')}
          </button>
        ))}
      </div>

      {error && (
        <div
          className="rounded-sm border border-error/50 bg-error/10 p-3 text-sm text-error"
          role="alert"
        >
          {errorMessages[error]}
        </div>
      )}
      {notice && (
        <div
          className="rounded-sm border border-success/50 bg-success/10 p-3 text-sm text-success"
          role="status"
          aria-live="polite"
        >
          {notice.kind === 'path'
            ? t('agentTools.nativePathCopied', { path: notice.path })
            : notice.kind === 'copyExtensions'
              ? t('agentTools.copyExtensions')
              : t('agentTools.copyWarning')}
        </div>
      )}

      {inventory === null ? (
        <div
          className="flex items-center gap-2 py-4 text-sm text-low"
          role="status"
        >
          {loading && (
            <SpinnerIcon
              className="size-icon-xs animate-spin"
              aria-hidden="true"
            />
          )}
          {loading ? t('agentTools.loading') : t('agentTools.unavailable')}
        </div>
      ) : (
        <div
          id="agent-tools-provider-list"
          className="space-y-4"
          role="tabpanel"
        >
          {inventories.map((providerInventory) => {
            const providerBlocked =
              actionDisabled || providerInventory.errors.length > 0;
            const items = providerInventory.items.filter(
              (item) => item.kind === kind
            );
            return (
              <section
                key={providerInventory.provider}
                className="rounded-sm border border-border bg-secondary/20"
              >
                <div className="flex items-center justify-between border-b border-border px-3 py-2">
                  <div>
                    <span className="text-sm font-medium text-high">
                      {PROVIDER_LABELS[providerInventory.provider]}
                    </span>
                    <span className="ml-2 text-xs text-low">
                      {providerInventory.installed
                        ? t('agentTools.installed')
                        : t('agentTools.notDetected')}
                    </span>
                  </div>
                  <span className="text-xs text-low">
                    {t('agentTools.itemCount', { count: items.length })}
                  </span>
                </div>

                {providerInventory.errors.map((providerError, index) => (
                  <div
                    key={`${index}:${providerError}`}
                    className="border-b border-error/30 bg-error/5 px-3 py-2 text-xs text-error"
                    role="alert"
                  >
                    {t('agentTools.nativeUnreadable')}
                  </div>
                ))}

                {providerInventory.limitations.map((limitation) => (
                  <div
                    key={limitation}
                    className="border-b border-border/60 px-3 py-2 text-xs text-low"
                  >
                    {t('agentTools.providerNoteOriginal', {
                      message: limitation,
                    })}
                  </div>
                ))}

                {items.length === 0 ? (
                  <div className="px-3 py-4 text-sm text-low">
                    {kind === 'mcp_server'
                      ? t('agentTools.noMcp')
                      : t('agentTools.noSkills')}
                  </div>
                ) : (
                  items.map((item) => {
                    const key = `${item.provider}:${item.scope}:${item.kind}:${item.name}`;
                    const busy = busyKey?.endsWith(
                      `:${item.provider}:${item.name}`
                    );
                    const hasCopyTarget = installedProviders.some(
                      (provider) => provider !== item.provider
                    );
                    return (
                      <div
                        key={`${key}:${item.native_path}`}
                        className="flex flex-col gap-2 border-b border-border/60 px-3 py-3 last:border-0"
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <div className="flex items-center gap-2">
                              <span className="truncate text-sm font-medium text-normal">
                                {item.name}
                              </span>
                              <span
                                className={cn(
                                  'rounded px-1.5 py-0.5 text-xs',
                                  item.state === 'enabled'
                                    ? 'bg-success/10 text-success'
                                    : item.state === 'error'
                                      ? 'bg-error/10 text-error'
                                      : 'bg-secondary text-low'
                                )}
                              >
                                {toolStates[item.state]}
                              </span>
                              <span className="rounded bg-secondary px-1.5 py-0.5 text-xs text-low">
                                {item.scope === 'user'
                                  ? t('agentTools.scopeUser')
                                  : t('agentTools.scopeProject')}
                              </span>
                            </div>
                            <p className="mt-1 truncate font-mono text-xs text-low">
                              {item.native_path}
                            </p>
                            {item.error && (
                              <p className="mt-1 text-xs text-error">
                                {t('agentTools.nativeInvalid')}
                              </p>
                            )}
                          </div>
                          {busy ? (
                            <span
                              role="status"
                              aria-label={t('agentTools.updating', {
                                name: item.name,
                              })}
                            >
                              <SpinnerIcon
                                className="size-icon-xs animate-spin text-low"
                                aria-hidden="true"
                              />
                            </span>
                          ) : (
                            item.capabilities.toggleable &&
                            ['enabled', 'disabled'].includes(item.state) && (
                              <Switch
                                checked={item.state === 'enabled'}
                                disabled={providerBlocked}
                                aria-label={t('agentTools.toggle', {
                                  name: item.name,
                                })}
                                onCheckedChange={(enabled) =>
                                  void run(
                                    `toggle:${item.provider}:${item.name}`,
                                    () =>
                                      machineClient!.toggleAgentTool({
                                        target: locatorFor(
                                          item,
                                          projectPath.trim()
                                        ),
                                        expected_revision: item.revision,
                                        enabled,
                                      })
                                  )
                                }
                              />
                            )
                          )}
                        </div>
                        <div className="flex flex-wrap gap-1">
                          <ToolButton
                            label={t('agentTools.read')}
                            icon={CodeIcon}
                            disabled={providerBlocked}
                            onClick={() => void readDefinition(item)}
                          />
                          <ToolButton
                            label={t('agentTools.edit')}
                            icon={PencilSimpleIcon}
                            disabled={
                              !item.capabilities.editable ||
                              item.state !== 'enabled' ||
                              providerBlocked
                            }
                            onClick={() => void handleEdit(item)}
                          />
                          <ToolButton
                            label={t('agentTools.copy')}
                            icon={CopyIcon}
                            disabled={
                              !item.capabilities.exportable ||
                              item.state !== 'enabled' ||
                              !hasCopyTarget ||
                              providerBlocked
                            }
                            onClick={() => void handleCopy(item)}
                          />
                          <ToolButton
                            label={t('agentTools.reveal')}
                            icon={FolderOpenIcon}
                            disabled={providerBlocked}
                            onClick={() => void handleReveal(item)}
                          />
                          <ToolButton
                            label={t('agentTools.remove')}
                            icon={TrashIcon}
                            danger
                            disabled={
                              !item.capabilities.removable || providerBlocked
                            }
                            onClick={() => {
                              if (
                                machineClient &&
                                window.confirm(
                                  t('agentTools.removeConfirm', {
                                    name: item.name,
                                  })
                                )
                              ) {
                                void run(
                                  `remove:${item.provider}:${item.name}`,
                                  () =>
                                    machineClient.removeAgentTool({
                                      target: locatorFor(
                                        item,
                                        projectPath.trim()
                                      ),
                                      expected_revision: item.revision,
                                    })
                                );
                              }
                            }}
                          />
                        </div>
                      </div>
                    );
                  })
                )}
              </section>
            );
          })}
        </div>
      )}
    </SettingsCard>
  );
}

function ToolButton({
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
        'inline-flex items-center gap-1 rounded-sm px-2 py-1 text-xs',
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
