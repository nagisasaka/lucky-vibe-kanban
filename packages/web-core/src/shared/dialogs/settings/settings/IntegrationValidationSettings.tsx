import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { MachineClient } from '@/shared/lib/machineClient';
import {
  editableCommands,
  validationCommands,
} from '@/shared/lib/integrationValidationSettings';
import type { IntegrationValidationSettings as Snapshot } from '@/shared/lib/integrationValidationSettings';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { SettingsCard, SettingsTextarea } from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';

const emptyPlan = '{\n  "commands": [\n    ""\n  ]\n}\n';

export function IntegrationValidationSettings({
  repoId,
  repoPath,
  client,
}: {
  repoId: string;
  repoPath: string;
  client: MachineClient;
}) {
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();
  const queryKey = [...client.queryScopeKey, 'integration-validation', repoId];
  const query = useQuery({
    queryKey,
    queryFn: () => client.getIntegrationValidation(repoId),
    refetchOnWindowFocus: false,
  });
  // Keep the revision the user actually edited, even if the query cache refreshes.
  const [draft, setDraft] = useState<{
    base: Snapshot;
    content: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const snapshot = draft?.base ?? query.data;
  const content = draft?.content ?? snapshot?.content ?? emptyPlan;
  const dirty = draft !== null && draft.content !== draft.base.content;
  const valid = validationCommands(content) !== null;
  const commands = editableCommands(content);
  const { setDirty } = useSettingsDirty();
  useEffect(() => {
    const key = `integration-validation:${client.target.id}:${repoId}`;
    setDirty(key, dirty);
    return () => setDirty(key, false);
  }, [client.target.id, repoId, dirty, setDirty]);

  const edit = (value: string) => {
    if (snapshot) setDraft({ base: snapshot, content: value });
    setSaved(false);
    setError(null);
  };
  const save = async () => {
    if (!snapshot || !valid || busy) return;
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      const result = await client.saveIntegrationValidation(repoId, {
        expected_revision: snapshot.revision,
        content,
      });
      queryClient.setQueryData(queryKey, result);
      setDraft(null);
      setSaved(true);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };
  const reload = async () => {
    setBusy(true);
    setError(null);
    setSaved(false);
    const result = await query.refetch();
    if (result.isSuccess) setDraft(null);
    setBusy(false);
  };
  return (
    <SettingsCard
      title={t('settings.integrationValidation.title')}
      description={t('settings.integrationValidation.description')}
    >
      <p className="text-sm text-low">
        {t('settings.integrationValidation.timing')}
      </p>
      <p className="text-sm text-low">
        {t('settings.integrationValidation.concurrency')}
      </p>
      <p className="break-all text-sm text-normal">
        {repoPath}/.lvk/integration-validation.json
      </p>
      {query.isLoading && (
        <p role="status">{t('settings.integrationValidation.loading')}</p>
      )}
      {query.isError && (
        <p role="alert" className="text-error">
          {t('settings.integrationValidation.loadError')} {query.error?.message}
        </p>
      )}
      {snapshot && (
        <>
          <p className="break-all text-sm text-normal">
            {snapshot.branch} · {snapshot.head}
          </p>
          {snapshot.content === null && (
            <p role="status">{t('settings.integrationValidation.missing')}</p>
          )}
          <details open={commands === null ? true : undefined}>
            <summary className="cursor-pointer text-sm text-normal">
              {t('settings.integrationValidation.json')}
            </summary>
            <label className="block text-sm text-normal">
              {t('settings.integrationValidation.json')}
              <SettingsTextarea
                value={content}
                onChange={edit}
                rows={8}
                monospace
                disabled={busy}
              />
            </label>
          </details>
          {!valid && (
            <p role="alert" className="text-error">
              {t('settings.integrationValidation.invalid')}
            </p>
          )}
          {commands?.map((command, index) => (
            <div key={index}>
              <label className="block text-sm text-normal">
                {t('settings.integrationValidation.command')} {index + 1}
                <SettingsTextarea
                  value={command}
                  disabled={busy}
                  monospace
                  onChange={(value) => {
                    const next = [...commands];
                    next[index] = value;
                    edit(JSON.stringify({ commands: next }, null, 2) + '\n');
                  }}
                />
              </label>
              <PrimaryButton
                className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:text-low"
                variant="tertiary"
                disabled={busy}
                onClick={() =>
                  edit(
                    JSON.stringify(
                      { commands: commands.filter((_, i) => i !== index) },
                      null,
                      2
                    ) + '\n'
                  )
                }
              >
                {t('settings.integrationValidation.remove')}
              </PrimaryButton>
            </div>
          ))}
          {commands && (
            <PrimaryButton
              className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:text-low"
              variant="tertiary"
              disabled={busy || commands.length >= 32}
              onClick={() =>
                edit(
                  JSON.stringify({ commands: [...commands, ''] }, null, 2) +
                    '\n'
                )
              }
            >
              {t('settings.integrationValidation.add')}
            </PrimaryButton>
          )}
        </>
      )}
      {error && (
        <p role="alert" className="text-error">
          {t('settings.integrationValidation.saveError')} {error}
        </p>
      )}
      {saved && (
        <p role="status">{t('settings.integrationValidation.saved')}</p>
      )}
      <div className="flex flex-wrap gap-base">
        <PrimaryButton
          className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:text-low"
          disabled={busy || !snapshot || !valid || !dirty || query.isError}
          onClick={() => void save()}
        >
          {t('settings.integrationValidation.save')}
        </PrimaryButton>
        <PrimaryButton
          className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:text-low"
          variant="tertiary"
          disabled={busy || query.isLoading}
          onClick={() => void reload()}
        >
          {t('settings.integrationValidation.reload')}
        </PrimaryButton>
      </div>
    </SettingsCard>
  );
}
