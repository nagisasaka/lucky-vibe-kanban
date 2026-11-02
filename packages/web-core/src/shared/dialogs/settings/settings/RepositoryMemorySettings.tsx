import { useTranslation } from 'react-i18next';
import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import type {
  Repo,
  RepositoryWikiStatus,
  OpenWikiBootstrapPhase,
} from 'shared/types';
import type { MachineClient } from '@/shared/lib/machineClient';
import { WorkspaceWorkflowLink } from '@/shared/components/WorkspaceWorkflowLink';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  SettingsCard,
  SettingsField,
  SettingsInput,
  SettingsCheckbox,
  SettingsSelect,
} from './SettingsComponents';

export function RepositoryMemorySettings({
  repo,
  client,
  projectId,
}: {
  repo: Repo;
  projectId?: string;
  client: MachineClient;
}) {
  const { t, i18n } = useTranslation('settings');
  const statuses: Record<RepositoryWikiStatus, string> = {
    disabled: t('repositoryMemory.status.disabled'),
    uninitialized: t('repositoryMemory.status.uninitialized'),
    initializing: t('repositoryMemory.status.initializing'),
    current: t('repositoryMemory.status.current'),
    stale: t('repositoryMemory.status.stale'),
    reconciling: t('repositoryMemory.status.reconciling'),
    error: t('repositoryMemory.status.error'),
  };
  const phases: Record<OpenWikiBootstrapPhase, string> = {
    generating: t('repositoryMemory.phase.generating'),
    reviewing: t('repositoryMemory.phase.reviewing'),
    refining: t('repositoryMemory.phase.refining'),
    publishing: t('repositoryMemory.phase.publishing'),
    cleaning_up: t('repositoryMemory.phase.cleaningUp'),
  };
  const checkpointMessage = (message: string) => {
    const identities =
      /^Workspace ([0-9a-f-]{36}) \/ run ([0-9a-f-]{36}):/i.exec(message);
    return identities
      ? t('repositoryMemory.checkpointIdentified', {
          workspace: identities[1],
          run: identities[2],
        })
      : t('repositoryMemory.checkpoint');
  };
  const query = useQuery({
    queryKey: [...client.queryScopeKey, 'repository-memory', repo.id],
    queryFn: () => client.getRepositoryMemory(repo.id),
    refetchInterval: 5000,
  });
  const [enabled, setEnabled] = useState(query.data?.enabled ?? true);
  const [branch, setBranch] = useState(
    query.data?.target_branch ?? repo.default_target_branch ?? 'main'
  );
  const [language, setLanguage] = useState(query.data?.output_language ?? 'en');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [scope, setScope] = useState<'current' | 'since'>('current');
  const [fromCommit, setFromCommit] = useState('');
  const state = query.data;
  const savedEnabled = state?.enabled;
  const savedBranch = state?.target_branch;
  const savedLanguage = state?.output_language;
  useEffect(() => {
    if (savedEnabled !== undefined && savedLanguage !== undefined) {
      setEnabled(savedEnabled);
      setBranch(savedBranch ?? repo.default_target_branch ?? 'main');
      setLanguage(savedLanguage);
    }
  }, [
    savedEnabled,
    savedBranch,
    savedLanguage,
    repo.id,
    repo.default_target_branch,
  ]);

  const active = state?.active_run_id != null || state?.bootstrap != null;
  const unsaved =
    state != null &&
    (enabled !== state.enabled ||
      branch !==
        (state.target_branch ?? repo.default_target_branch ?? 'main') ||
      language !== state.output_language);
  const needsCommit = state?.wiki_exists && scope === 'since';
  const invalidCommit =
    needsCommit && !/^[a-fA-F0-9]{4,40}$/.test(fromCommit.trim());
  const cannotStart =
    busy ||
    active ||
    !state?.enabled ||
    unsaved ||
    query.isError ||
    invalidCommit;
  const perform = async (sync: boolean) => {
    // Sync uses the server's saved branch, not the draft displayed in this form.
    if (sync && cannotStart) return;
    setBusy(true);
    setError(false);
    try {
      if (sync)
        await client.syncRepositoryMemory(repo.id, {
          ...(projectId ? { project_id: projectId } : {}),
          from_commit: needsCommit ? fromCommit.trim() : null,
        });
      else
        await client.configureRepositoryMemory(repo.id, {
          enabled,
          target_branch: branch,
          output_language: language,
        });
      await query.refetch();
    } catch {
      setError(true);
      await query.refetch();
    } finally {
      setBusy(false);
    }
  };
  return (
    <SettingsCard
      title={t('repositoryMemory.title')}
      description={t('repositoryMemory.description')}
    >
      <p className="text-sm text-normal" role="status">
        {state?.bootstrap
          ? t('repositoryMemory.stateWithPhase', {
              status: statuses[state.status],
              phase: phases[state.bootstrap.phase],
            })
          : t('repositoryMemory.state', {
              status: state
                ? statuses[state.status]
                : t('repositoryMemory.loading'),
            })}
      </p>
      <SettingsCheckbox
        id={`openwiki-enabled-${repo.id}`}
        label={t('repositoryMemory.enabled')}
        checked={enabled}
        onChange={setEnabled}
        disabled={busy || active}
      />
      <SettingsField
        label={t('repositoryMemory.targetBranch')}
        htmlFor={`openwiki-branch-${repo.id}`}
      >
        <SettingsInput
          id={`openwiki-branch-${repo.id}`}
          value={branch}
          onChange={setBranch}
          disabled={busy || active}
        />
      </SettingsField>
      <SettingsField
        label={t('repositoryMemory.outputLanguage')}
        htmlFor={`openwiki-language-${repo.id}`}
      >
        <SettingsInput
          id={`openwiki-language-${repo.id}`}
          value={language}
          onChange={setLanguage}
          placeholder="en, ja, fr, zh-Hans"
          disabled={busy || active}
        />
      </SettingsField>
      {state?.wiki_exists && (
        <>
          <p className="text-sm text-low">
            {t('repositoryMemory.existingWiki', {
              branch: state.target_branch,
            })}
          </p>
          <SettingsField
            label={t('repositoryMemory.scope')}
            htmlFor={`openwiki-scope-${repo.id}`}
          >
            <SettingsSelect
              id={`openwiki-scope-${repo.id}`}
              value={scope}
              onChange={setScope}
              disabled={busy || active}
              options={[
                {
                  value: 'current',
                  label: t('repositoryMemory.compareCurrent', {
                    branch: state.target_branch ?? t('repositoryMemory.branch'),
                  }),
                },
                { value: 'since', label: t('repositoryMemory.sinceCommit') },
              ]}
            />
          </SettingsField>
          {needsCommit && (
            <SettingsField
              label={t('repositoryMemory.startingCommit')}
              htmlFor={`openwiki-commit-${repo.id}`}
              description={t('repositoryMemory.startingCommitHint')}
              error={invalidCommit ? t('repositoryMemory.invalidCommit') : null}
            >
              <SettingsInput
                id={`openwiki-commit-${repo.id}`}
                error={Boolean(invalidCommit)}
                value={fromCommit}
                onChange={setFromCommit}
                placeholder={t('repositoryMemory.commitPlaceholder')}
                disabled={busy || active}
              />
            </SettingsField>
          )}
        </>
      )}
      <div className="flex flex-wrap gap-base">
        <PrimaryButton
          value={t('repositoryMemory.save')}
          disabled={busy || active || !state}
          onClick={() => void perform(false)}
        />
        <PrimaryButton
          value={
            state?.wiki_exists
              ? state.last_success
                ? t('repositoryMemory.sync')
                : t('repositoryMemory.adopt')
              : t('repositoryMemory.initialize')
          }
          disabled={cannotStart}
          onClick={() => void perform(true)}
        />
      </div>
      {!state?.last_success && (
        <p className="text-sm text-low">{t('repositoryMemory.startHint')}</p>
      )}
      {unsaved && (
        <p className="text-sm text-low" role="status">
          {t('repositoryMemory.unsaved')}
        </p>
      )}
      <p className="text-sm text-low">
        {t('repositoryMemory.lastSuccess', {
          date: state?.last_success
            ? new Date(state.last_success).toLocaleString(
                i18n.resolvedLanguage || i18n.language
              )
            : t('repositoryMemory.none'),
        })}
      </p>
      {state?.unverified_before && (
        <p className="break-all text-sm text-low" role="status">
          {t('repositoryMemory.unverifiedBefore', {
            commit: state.unverified_before,
          })}
        </p>
      )}
      {state?.source_commit && (
        <p className="break-all text-sm text-low">
          {t('repositoryMemory.integratedSource', {
            commit: state.source_commit,
          })}
        </p>
      )}
      {state?.maintenance_workspace_id && (
        <div className="space-y-half">
          <WorkspaceWorkflowLink
            workspaceId={state.maintenance_workspace_id}
            hostId={client.target.apiHostId}
          />
          <a
            className="block break-all text-sm text-brand underline"
            href={`${client.target.apiHostId ? `/hosts/${client.target.apiHostId}` : ''}/workspaces/${state.maintenance_workspace_id}`}
          >
            {t('repositoryMemory.workspace', {
              id: state.maintenance_workspace_id,
            })}
          </a>
        </div>
      )}
      {(error || query.error || state?.error) && (
        <p className="whitespace-pre-wrap text-sm text-error" role="alert">
          {error
            ? t('repositoryMemory.operationUnconfirmed')
            : query.error
              ? t('repositoryMemory.loadFailed')
              : t('repositoryMemory.maintenanceError')}
        </p>
      )}
      {state?.coding_errors?.map((message) => (
        <p
          key={message}
          className="whitespace-pre-wrap text-sm text-error"
          role="alert"
        >
          {checkpointMessage(message)}
        </p>
      ))}
      {state?.status === 'error' && (
        <p className="text-sm text-low">{t('repositoryMemory.errorHint')}</p>
      )}
    </SettingsCard>
  );
}
