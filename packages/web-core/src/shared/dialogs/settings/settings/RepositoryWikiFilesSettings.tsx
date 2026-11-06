import { useCallback, useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Repo, RepositoryWikiFile } from 'shared/types';
import type { MachineClient } from '@/shared/lib/machineClient';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { SettingsField, SettingsInput } from './SettingsComponents';

function FileEditor({
  file,
  disabled,
  onSave,
  onDirty,
}: {
  file: RepositoryWikiFile;
  disabled: boolean;
  onSave: (path: string, value: string) => Promise<void>;
  onDirty: (path: string, dirty: boolean) => void;
}) {
  const { t } = useTranslation('settings');
  const [value, setValue] = useState(file.content);
  const [editing, setEditing] = useState(false);
  const dirty = value !== file.content;
  useEffect(() => {
    setValue(file.content);
  }, [file.content]);
  useEffect(() => {
    onDirty(file.path, dirty);
  }, [file.path, dirty, onDirty]);
  return (
    <div className="space-y-base">
      <SettingsField
        label={file.path}
        htmlFor={`wiki-file-${file.path}`}
        description={
          file.exists
            ? t('repositoryMemory.files.existing')
            : t('repositoryMemory.files.defaultValue')
        }
      >
        <textarea
          id={`wiki-file-${file.path}`}
          aria-label={file.path}
          value={value}
          readOnly={!editing || disabled}
          onChange={(event) => setValue(event.target.value)}
          rows={file.path.endsWith('.md') ? 12 : 5}
          className="w-full rounded border border-border bg-primary p-base font-ibm-plex-mono text-sm text-normal focus:outline-none focus:ring-1 focus:ring-brand"
        />
      </SettingsField>
      <div className="flex flex-wrap gap-base">
        <PrimaryButton
          value={t('repositoryMemory.files.edit')}
          variant="tertiary"
          disabled={disabled || editing}
          onClick={() => setEditing(true)}
        />
        <PrimaryButton
          value={t('repositoryMemory.files.reset')}
          variant="tertiary"
          disabled={disabled}
          onClick={() => {
            setEditing(true);
            setValue(file.default_content);
          }}
        />
        <PrimaryButton
          value={t('repositoryMemory.save')}
          disabled={disabled || (!dirty && file.exists)}
          onClick={() => void onSave(file.path, value)}
        />
      </div>
    </div>
  );
}

export function RepositoryWikiFilesSettings({
  repo,
  client,
  disabled,
  wikiExists,
  enabled,
  branch,
  projectId,
  onDirtyChange,
  refresh,
}: {
  repo: Repo;
  client: MachineClient;
  disabled: boolean;
  wikiExists: boolean;
  enabled: boolean;
  branch: string | null | undefined;
  projectId?: string;
  onDirtyChange: (dirty: boolean) => void;
  refresh: () => Promise<unknown>;
}) {
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();
  const key = [
    ...client.queryScopeKey,
    'repository-wiki-files',
    repo.id,
    branch,
  ];
  const query = useQuery({
    queryKey: key,
    queryFn: () => client.getRepositoryWikiFiles(repo.id),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
  const [dirtyFiles, setDirtyFiles] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmation, setConfirmation] = useState('');
  const dirty = Object.values(dirtyFiles).some(Boolean);
  useEffect(() => {
    onDirtyChange(dirty || busy);
  }, [dirty, busy, onDirtyChange]);
  const recordDirty = useCallback(
    (path: string, changed: boolean) =>
      setDirtyFiles((previous) =>
        previous[path] === changed ? previous : { ...previous, [path]: changed }
      ),
    []
  );
  const data = query.data;
  const blocked = disabled || busy || !data || query.isError;
  const perform = async (
    action: 'save' | 'delete' | 'rebuild',
    path?: string,
    content?: string
  ) => {
    if (blocked || !data || (action !== 'save' && dirty)) return;
    if (action === 'delete' && confirmation !== `${repo.name}/openwiki`) return;
    setBusy(true);
    setError(null);
    try {
      const target = { branch: data.branch, source: data.source, confirmation };
      if (action === 'save' && path !== undefined && content !== undefined) {
        const saved = await client.saveRepositoryWikiFile(repo.id, {
          branch: data.branch,
          source: data.source,
          path,
          content,
        });
        queryClient.setQueryData(key, saved);
      } else if (action === 'delete') {
        await client.deleteRepositoryWiki(repo.id, target);
        setConfirmation('');
        await query.refetch();
      } else if (action === 'rebuild') {
        await client.rebuildRepositoryWiki(repo.id, {
          branch: data.branch,
          source: data.source,
          ...(projectId ? { project_id: projectId } : {}),
        });
      }
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      await refresh();
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-double border-t border-border pt-double">
      <p className="text-sm text-normal">
        {t('repositoryMemory.files.target', {
          repository: repo.name,
          branch: data?.branch ?? '',
        })}
      </p>
      <p className="text-sm text-low">{t('repositoryMemory.files.saveHint')}</p>
      {data?.files.map((file) => (
        <FileEditor
          key={`${repo.id}/${file.path}`}
          file={file}
          disabled={blocked}
          onDirty={recordDirty}
          onSave={(path, value) => perform('save', path, value)}
        />
      ))}
      <p className="text-sm text-low">
        {t('repositoryMemory.files.ignoreHint')}
      </p>
      {(query.error || error) && (
        <p role="alert" className="whitespace-pre-wrap text-sm text-error">
          {t('repositoryMemory.operationUnconfirmed')}{' '}
          {error ?? query.error?.message}
        </p>
      )}
      {wikiExists && (
        <div className="space-y-double border-t border-error pt-double">
          <h4 className="text-base font-medium text-error">
            {t('repositoryMemory.files.danger')}
          </h4>
          <div className="space-y-base">
            <p className="text-sm text-normal">
              {t('repositoryMemory.files.rebuildHint')}
            </p>
            <PrimaryButton
              value={t('repositoryMemory.files.rebuild')}
              disabled={blocked || dirty || !enabled}
              onClick={() => void perform('rebuild')}
            />
          </div>
          <div className="space-y-base">
            <p className="text-sm text-normal">
              {t('repositoryMemory.files.deleteHint', {
                repository: repo.name,
                branch: data?.branch ?? '',
              })}
            </p>
            <SettingsField
              label={t('repositoryMemory.files.confirm', {
                target: `${repo.name}/openwiki`,
              })}
              htmlFor={`wiki-delete-${repo.id}`}
            >
              <SettingsInput
                id={`wiki-delete-${repo.id}`}
                value={confirmation}
                onChange={setConfirmation}
                disabled={blocked}
              />
            </SettingsField>
            <PrimaryButton
              value={t('repositoryMemory.files.delete')}
              className={
                blocked || dirty || confirmation !== `${repo.name}/openwiki`
                  ? undefined
                  : 'bg-error text-on-brand hover:bg-error/90'
              }
              disabled={
                blocked || dirty || confirmation !== `${repo.name}/openwiki`
              }
              onClick={() => void perform('delete')}
            />
          </div>
        </div>
      )}
    </div>
  );
}
