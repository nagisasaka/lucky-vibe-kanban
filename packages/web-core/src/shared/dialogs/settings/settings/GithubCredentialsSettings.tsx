import { useTranslation } from 'react-i18next';
import { ApiError } from '@/shared/lib/api';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { MachineClient } from '@/shared/lib/machineClient';
import { Button } from '@vibe/ui/components/Button';
import { SettingsCard } from './SettingsComponents';

export function GithubCredentialsSettings({
  repoId,
  client,
}: {
  repoId: string;
  client: MachineClient;
}) {
  const { t } = useTranslation('settings');
  const query = useQuery({
    queryKey: [...client.queryScopeKey, 'github-credentials', repoId],
    queryFn: () => client.getGithubCredentials(repoId),
  });
  const [selected, setSelected] = useState('');
  const [token, setToken] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<
    'saved' | 'checked' | 'removed' | null
  >(null);
  const [error, setError] = useState<'forbidden' | 'unconfirmed' | null>(null);
  const repository = selected || query.data?.[0]?.repository || '';
  const configured =
    query.data?.find((entry) => entry.repository === repository)?.configured ??
    false;
  const perform = async (action: 'save' | 'check' | 'remove') => {
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      if (action === 'save') {
        await client.saveGithubCredential(repoId, { repository, token });
        setToken('');
        setMessage('saved');
      } else if (action === 'check') {
        await client.checkGithubCredential(repoId, { repository });
        setMessage('checked');
      } else {
        await client.removeGithubCredential(repoId, { repository });
        setToken('');
        setMessage('removed');
      }
      await query.refetch();
    } catch (cause) {
      setError(
        cause instanceof ApiError && cause.statusCode === 403
          ? 'forbidden'
          : 'unconfirmed'
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <SettingsCard
      title={t('githubCredentials.title')}
      description={t('githubCredentials.description')}
    >
      <p className="text-sm text-normal">
        {t('githubCredentials.permissions')}
      </p>
      <a
        className="text-sm text-brand underline"
        href="https://github.com/settings/personal-access-tokens/new"
        target="_blank"
        rel="noreferrer"
      >
        {t('githubCredentials.createToken')}
      </a>
      {query.isLoading && <p role="status">{t('githubCredentials.loading')}</p>}
      {query.isError && (
        <p role="alert" className="text-error">
          {t('githubCredentials.loadFailed')}
        </p>
      )}
      {query.data?.length === 0 && (
        <p className="text-sm text-low">{t('githubCredentials.empty')}</p>
      )}
      {repository && (
        <>
          <label className="block text-sm text-normal">
            {t('githubCredentials.repository')}
            <select
              className="mt-half block w-full rounded-sm border border-border bg-panel p-base"
              value={repository}
              disabled={busy}
              onChange={(event) => {
                setSelected(event.target.value);
                setToken('');
                setMessage(null);
                setError(null);
              }}
            >
              {query.data?.map((entry) => (
                <option key={entry.repository} value={entry.repository}>
                  {entry.repository}
                </option>
              ))}
            </select>
          </label>
          <p role="status" className="text-sm text-low">
            {configured
              ? t('githubCredentials.registered')
              : t('githubCredentials.notRegistered')}
          </p>
          <label className="block text-sm text-normal">
            {configured
              ? t('githubCredentials.replacement')
              : t('githubCredentials.token')}
            <input
              type="password"
              autoComplete="new-password"
              spellCheck={false}
              value={token}
              disabled={busy}
              onChange={(event) => setToken(event.target.value)}
              placeholder="github_pat_…"
              className="mt-half block w-full rounded-sm border border-border bg-panel p-base"
            />
          </label>
          <div className="flex flex-wrap gap-base">
            <Button
              disabled={
                busy || query.isError || !token.trim().startsWith('github_pat_')
              }
              onClick={() => void perform('save')}
            >
              {configured
                ? t('githubCredentials.replace')
                : t('githubCredentials.save')}
            </Button>
            <Button
              variant="outline"
              disabled={busy || !configured || query.isError || !!token}
              onClick={() => void perform('check')}
            >
              {t('githubCredentials.check')}
            </Button>
            <Button
              variant="outline"
              disabled={busy || !configured || query.isError}
              onClick={() => {
                if (window.confirm(t('githubCredentials.removeConfirm')))
                  void perform('remove');
              }}
            >
              {t('githubCredentials.remove')}
            </Button>
          </div>
        </>
      )}
      <p className="text-sm text-low">{t('githubCredentials.scope')}</p>
      {busy && (
        <p role="status" className="text-sm text-low">
          {t('githubCredentials.working')}
        </p>
      )}
      {message && (
        <p role="status" className="text-sm text-success">
          {
            {
              saved: t('githubCredentials.saved'),
              checked: t('githubCredentials.checked'),
              removed: t('githubCredentials.removed'),
            }[message]
          }
        </p>
      )}
      {error && (
        <p role="alert" className="text-sm text-error">
          {error === 'forbidden'
            ? t('githubCredentials.forbidden')
            : t('githubCredentials.operationFailed')}
        </p>
      )}
    </SettingsCard>
  );
}
