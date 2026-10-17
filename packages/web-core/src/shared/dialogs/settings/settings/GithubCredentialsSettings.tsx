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
  const query = useQuery({
    queryKey: [...client.queryScopeKey, 'github-credentials', repoId],
    queryFn: () => client.getGithubCredentials(repoId),
  });
  const [selected, setSelected] = useState('');
  const [token, setToken] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const repository = selected || query.data?.[0]?.repository || '';
  const configured =
    query.data?.find((entry) => entry.repository === repository)?.configured ??
    false;
  const perform = async (action: 'save' | 'check' | 'remove') => {
    setBusy(true);
    setMessage('');
    setError('');
    try {
      if (action === 'save') {
        await client.saveGithubCredential(repoId, { repository, token });
        setToken('');
        setMessage(
          'Token saved. Use Check connection to verify repository and PR read access.'
        );
      } else if (action === 'check') {
        await client.checkGithubCredential(repoId, { repository });
        setMessage(
          'Repository and PR read access verified. Push and PR write permissions have not been tested.'
        );
      } else {
        await client.removeGithubCredential(repoId, { repository });
        setToken('');
        setMessage(
          'Token removed from LVK. Revoke it on GitHub if it is no longer needed.'
        );
      }
      await query.refetch();
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : 'GitHub credential operation failed'
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <SettingsCard
      title="GitHub authentication"
      description="Register a fine-grained personal access token for LVK Git operations and pull requests. Administrator access is required to change or check credentials."
    >
      <p className="text-sm text-normal">
        Select only the required repository when creating the token. Grant
        Contents and Pull requests read/write access. Add Issues for issue
        operations, and Workflows if you push workflow changes. Expiring and
        non-expiring tokens are supported, subject to your organisation’s
        policy.
      </p>
      <a
        className="text-sm text-brand underline"
        href="https://github.com/settings/personal-access-tokens/new"
        target="_blank"
        rel="noreferrer"
      >
        Create a fine-grained token on GitHub
      </a>
      {query.isLoading && <p role="status">Loading authentication settings…</p>}
      {query.isError && (
        <p role="alert" className="text-error">
          Could not load authentication settings.
        </p>
      )}
      {query.data?.length === 0 && (
        <p className="text-sm text-low">
          Add a GitHub.com remote to this repository to configure a token.
        </p>
      )}
      {repository && (
        <>
          <label className="block text-sm text-normal">
            GitHub repository
            <select
              className="mt-half block w-full rounded-sm border border-border bg-panel p-base"
              value={repository}
              disabled={busy}
              onChange={(event) => {
                setSelected(event.target.value);
                setToken('');
                setMessage('');
                setError('');
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
              ? 'Token registered (never displayed).'
              : 'No LVK token registered. Existing system authentication applies.'}
          </p>
          <label className="block text-sm text-normal">
            {configured
              ? 'Replacement token'
              : 'Fine-grained personal access token'}
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
              {configured ? 'Replace token' : 'Save token'}
            </Button>
            <Button
              variant="outline"
              disabled={busy || !configured || query.isError || !!token}
              onClick={() => void perform('check')}
            >
              Check connection
            </Button>
            <Button
              variant="outline"
              disabled={busy || !configured || query.isError}
              onClick={() => {
                if (
                  window.confirm(
                    'Remove this token from LVK? All workspaces using this GitHub repository will return to system authentication. This does not revoke the token on GitHub.'
                  )
                )
                  void perform('remove');
              }}
            >
              Remove token
            </Button>
          </div>
        </>
      )}
      <p className="text-sm text-low">
        Credentials are stored on the LVK server and shared by workspaces using
        this GitHub repository. Registered tokens take precedence over system
        authentication; failures do not fall back to another account. Agent
        terminal commands use their own authentication.
      </p>
      {busy && (
        <p role="status" className="text-sm text-low">
          Working…
        </p>
      )}
      {message && (
        <p role="status" className="text-sm text-success">
          {message}
        </p>
      )}
      {error && (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      )}
    </SettingsCard>
  );
}
