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
  const label = (key: string) => t(`settings.repositoryPush.${key}`);
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
      setError(error instanceof Error ? error.message : label('unknown'));
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
            : label('unknown')
        );
        return;
      }
      setError(error instanceof Error ? error.message : label('unknown'));
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
      setError(error instanceof Error ? error.message : label('unknown'));
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
    <SettingsCard title={label('title')} description={label('description')}>
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
            <option value="">{label('choose')}</option>
            {projects.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </SettingsField>
      )}
      <SettingsField label={label('branch')} htmlFor="repository-push-branch">
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
          <option value="">{label('choose')}</option>
          {branches.data
            ?.filter((item) => !item.is_remote)
            .map((item) => (
              <option key={item.name} value={item.name}>
                {item.name}
              </option>
            ))}
        </select>
      </SettingsField>
      <SettingsField label={label('remote')} htmlFor="repository-push-remote">
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
          <option value="">{label('choose')}</option>
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
          {label('error')} {error}
        </p>
      )}
      <PrimaryButton
        value={label('preview')}
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
            {label('source')}: <code>{shownPreview.source_sha}</code>
          </p>
          <p>
            {label('destination')}: {shownPreview.destination_branch}
          </p>
          <p>
            {label('remoteSha')}:{' '}
            <code>{shownPreview.remote_sha ?? label('absent')}</code>
          </p>
          <p>{label('history')}</p>
          <p>{shownPreview.unpublished_commits} commits</p>
          {shownPreview.issue_archive_paths.length > 0 && (
            <ul className="list-disc pl-double">
              {shownPreview.issue_archive_paths.map((path) => (
                <li key={path}>{path}</li>
              ))}
            </ul>
          )}
          <SettingsCheckbox
            id="repository-push-confirm"
            label={label('confirm')}
            checked={confirmed}
            onChange={setConfirmed}
            disabled={locked}
          />
          <PrimaryButton
            value={label('push')}
            onClick={push}
            disabled={locked || !confirmed}
          />
        </div>
      )}
      {requestId && (
        <div className="space-y-base text-sm text-normal" aria-live="polite">
          <p className="break-all">
            {label('request')}: {requestId}
          </p>
          <p>
            {label('status')}: {receipt?.status ?? label('unknown')}
          </p>
          {receipt?.result && <p>{receipt.result.outcome}</p>}
          {observed !== null && (
            <p>{label(observed ? 'published' : 'unpublished')}</p>
          )}
          <PrimaryButton
            value={label('recheck')}
            onClick={verify}
            disabled={busy}
          />
          {canReviewAnother && (
            <PrimaryButton
              variant="tertiary"
              value={label('newReview')}
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
