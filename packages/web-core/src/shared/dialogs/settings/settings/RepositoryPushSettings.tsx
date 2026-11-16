import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type {
  Repo,
  RepositoryPushPreview,
  RepositoryPushReceipt,
} from 'shared/types';
import { ApiError } from '@/shared/lib/api';
import type { MachineClient } from '@/shared/lib/machineClient';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  SettingsCard,
  SettingsCheckbox,
  SettingsField,
} from './SettingsComponents';

export function RepositoryPushSettings({
  repo,
  client,
  projectId,
  projects,
}: {
  repo: Repo;
  client: MachineClient;
  projectId?: string;
  projects: { id: string; name: string }[];
}) {
  const { t } = useTranslation('settings');
  const [project, setProject] = useState(projectId ?? '');
  const [branch, setBranch] = useState('');
  const [remote, setRemote] = useState('');
  const [preview, setPreview] = useState<RepositoryPushPreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const storageKey = `repository-push:${client.target.id}:${repo.id}`;
  const [requestId, setRequestId] = useState<string | null>(() => {
    try {
      return sessionStorage.getItem(storageKey);
    } catch {
      return null;
    }
  });
  const [receipt, setReceipt] = useState<RepositoryPushReceipt | null>(null);
  const [observed, setObserved] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const branches = useQuery({
    queryKey: [...client.queryScopeKey, 'push-branches', repo.id],
    queryFn: () => client.getRepoBranches(repo.id),
  });
  const remotes = useQuery({
    queryKey: [...client.queryScopeKey, 'push-remotes', repo.id],
    queryFn: () => client.listRepoRemotes(repo.id),
  });
  const invalidate = () => {
    setPreview(null);
    setConfirmed(false);
  };
  const review = async () => {
    setBusy(true);
    setError(null);
    invalidate();
    try {
      setPreview(
        await client.previewRepositoryPush(repo.id, {
          project_id: project,
          branch,
          remote,
        })
      );
    } catch (error) {
      setError(
        error instanceof Error
          ? error.message
          : t('settings.repositoryPush.unknown')
      );
    } finally {
      setBusy(false);
    }
  };
  const push = async () => {
    if (!preview || !confirmed || requestId) return;
    const id = crypto.randomUUID();
    try {
      sessionStorage.setItem(storageKey, id);
    } catch {
      /* The request ID remains visible in this session. */
    }
    setRequestId(id);
    setBusy(true);
    setError(null);
    try {
      setReceipt(
        await client.pushRepository(repo.id, { request_id: id, preview })
      );
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.error_data &&
        typeof error.error_data === 'object' &&
        'not_admitted' in error.error_data &&
        error.error_data.not_admitted === true
      ) {
        try {
          sessionStorage.removeItem(storageKey);
        } catch {
          /* No operation was admitted. */
        }
        setRequestId(null);
        invalidate();
        setError(
          'outcome' in error.error_data &&
            typeof error.error_data.outcome === 'string'
            ? error.error_data.outcome
            : t('settings.repositoryPush.unknown')
        );
        return;
      }
      setError(
        error instanceof Error
          ? error.message
          : t('settings.repositoryPush.unknown')
      );
    } finally {
      setBusy(false);
    }
  };
  const verify = async () => {
    if (!requestId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await client.verifyRepositoryPush(repo.id, requestId);
      setReceipt(result.receipt);
      setObserved(result.source_present);
      setError(result.verification_error);
    } catch (error) {
      setError(
        error instanceof Error
          ? error.message
          : t('settings.repositoryPush.unknown')
      );
    } finally {
      setBusy(false);
    }
  };
  const selectClass =
    'w-full rounded-sm border border-border bg-secondary p-half text-normal';
  const locked = busy || requestId !== null;
  const shownPreview = preview ?? receipt?.preview;
  const canReviewAnother =
    (observed !== null &&
      (receipt?.status === 'succeeded' || receipt?.status === 'failed')) ||
    (observed === true && receipt?.status === 'uncertain');
  return (
    <SettingsCard
      title={t('settings.repositoryPush.title')}
      description={t('settings.repositoryPush.description')}
    >
      <p className="break-all text-sm text-normal">
        {repo.display_name} · {repo.id} · {repo.path}
      </p>
      {!projectId && (
        <SettingsField label="Project" htmlFor="repository-push-project">
          <select
            id="repository-push-project"
            className={selectClass}
            value={project}
            disabled={locked}
            onChange={(e) => {
              setProject(e.target.value);
              invalidate();
            }}
          >
            <option value="">{t('settings.repositoryPush.choose')}</option>
            {projects.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </SettingsField>
      )}
      <SettingsField
        label={t('settings.repositoryPush.branch')}
        htmlFor="repository-push-branch"
      >
        <select
          id="repository-push-branch"
          className={selectClass}
          value={branch}
          disabled={locked}
          onChange={(e) => {
            setBranch(e.target.value);
            invalidate();
          }}
        >
          <option value="">{t('settings.repositoryPush.choose')}</option>
          {branches.data
            ?.filter((item) => !item.is_remote)
            .map((item) => (
              <option key={item.name} value={item.name}>
                {item.name}
              </option>
            ))}
        </select>
      </SettingsField>
      <SettingsField
        label={t('settings.repositoryPush.remote')}
        htmlFor="repository-push-remote"
      >
        <select
          id="repository-push-remote"
          className={selectClass}
          value={remote}
          disabled={locked}
          onChange={(e) => {
            setRemote(e.target.value);
            invalidate();
          }}
        >
          <option value="">{t('settings.repositoryPush.choose')}</option>
          {Array.from(new Set(remotes.data?.map((item) => item.name))).map(
            (name) => (
              <option key={name} value={name}>
                {name}
              </option>
            )
          )}
        </select>
      </SettingsField>
      {(error || branches.isError || remotes.isError) && (
        <p role="alert" className="text-sm text-error">
          {t('settings.repositoryPush.error')} {error}
        </p>
      )}
      <PrimaryButton
        value={t('settings.repositoryPush.preview')}
        onClick={review}
        disabled={locked || !project || !branch || !remote}
      />
      {shownPreview && (
        <div className="space-y-base break-all text-sm text-normal">
          <p>
            {shownPreview.repository_name} · {shownPreview.repository_id}
          </p>
          <p>
            {shownPreview.remote} · {shownPreview.remote_url}
          </p>
          <p>
            {t('settings.repositoryPush.source')}:{' '}
            <code>{shownPreview.source_sha}</code>
          </p>
          <p>
            {t('settings.repositoryPush.destination')}:{' '}
            {shownPreview.destination_branch}
          </p>
          <p>
            {t('settings.repositoryPush.remoteSha')}:{' '}
            <code>
              {shownPreview.remote_sha ?? t('settings.repositoryPush.absent')}
            </code>
          </p>
          <p>{t('settings.repositoryPush.history')}</p>
          <p>
            {t('settings.repositoryPush.commitCount', {
              count: shownPreview.unpublished_commits,
            })}
          </p>
          {shownPreview.issue_archive_paths.length > 0 && (
            <ul className="list-disc pl-double">
              {shownPreview.issue_archive_paths.map((path) => (
                <li key={path}>{path}</li>
              ))}
            </ul>
          )}
          <SettingsCheckbox
            id="repository-push-confirm"
            label={t('settings.repositoryPush.confirm')}
            checked={confirmed}
            onChange={setConfirmed}
            disabled={locked}
          />
          <PrimaryButton
            value={t('settings.repositoryPush.push')}
            onClick={push}
            disabled={locked || !confirmed}
          />
        </div>
      )}
      {requestId && (
        <div className="space-y-base text-sm text-normal" aria-live="polite">
          <p className="break-all">
            {t('settings.repositoryPush.request')}: {requestId}
          </p>
          <p>
            {t('settings.repositoryPush.status')}:{' '}
            {receipt?.status ?? t('settings.repositoryPush.unknown')}
          </p>
          {receipt?.result && <p>{receipt.result.outcome}</p>}
          {observed !== null && (
            <p>
              {observed
                ? t('settings.repositoryPush.published')
                : t('settings.repositoryPush.unpublished')}
            </p>
          )}
          <PrimaryButton
            value={t('settings.repositoryPush.recheck')}
            onClick={verify}
            disabled={busy}
          />
          {canReviewAnother && (
            <PrimaryButton
              variant="tertiary"
              value={t('settings.repositoryPush.newReview')}
              disabled={busy}
              onClick={() => {
                try {
                  sessionStorage.removeItem(storageKey);
                } catch {
                  /* Keep the visible result until explicitly reset. */
                }
                setRequestId(null);
                setReceipt(null);
                setObserved(null);
                invalidate();
              }}
            />
          )}
        </div>
      )}
    </SettingsCard>
  );
}
