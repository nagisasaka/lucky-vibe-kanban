import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { RuntimeAccessSettings, RuntimeAccessView } from 'shared/types';
import { runtimeAccessSettings } from '@/shared/lib/runtimeAccessApi';
import {
  SettingsCard,
  SettingsCheckbox,
  SettingsTextarea,
  SettingsSaveBar,
} from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';

export function RuntimeAccessSettingsSection() {
  const { t } = useTranslation('settings');
  const query = useQuery({
    queryKey: ['runtime-access-settings'],
    queryFn: () => runtimeAccessSettings(),
    retry: false,
  });
  const [draft, setDraft] = useState<RuntimeAccessView | null>(null);
  const [rootsText, setRootsText] = useState<string | null>(null);
  const [rootsOpen, setRootsOpen] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const { setDirty } = useSettingsDirty();
  const current = query.error ? undefined : (draft ?? query.data);
  useEffect(() => {
    setDirty('mcp-integration', !!draft);
    return () => setDirty('mcp-integration', false);
  }, [draft, setDirty]);
  useEffect(() => {
    if (query.data && query.data.settings.file_roots.join('\n') !== '/')
      setRootsOpen(true);
  }, [query.data]);
  function patch(change: Partial<RuntimeAccessSettings>) {
    if (current)
      setDraft({ ...current, settings: { ...current.settings, ...change } });
  }
  async function save() {
    if (!current) return;
    setBusy(true);
    setError('');
    try {
      await runtimeAccessSettings({
        revision: current.revision,
        settings: current.settings,
      });
      await query.refetch();
      setDraft(null);
      setRootsText(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <SettingsCard
      title={t('runtimeAccess.title')}
      description={t('runtimeAccess.description')}
    >
      {(error || query.error) && (
        <p role="alert" className="text-sm text-error">
          {error || t('runtimeAccess.denied')}
        </p>
      )}
      {query.isLoading && <p role="status">{t('runtimeAccess.loading')}</p>}
      {current && (
        <div className="space-y-4">
          <SettingsCheckbox
            id="runtime-files"
            label={t('runtimeAccess.files')}
            checked={current.settings.files_enabled}
            disabled={busy}
            onChange={(files_enabled) => patch({ files_enabled })}
          />
          <SettingsCheckbox
            id="runtime-execution"
            label={t('runtimeAccess.execute')}
            checked={current.settings.execution_enabled}
            disabled={busy}
            onChange={(execution_enabled) => patch({ execution_enabled })}
          />
          <details
            className="space-y-3 text-sm text-low"
            open={rootsOpen}
            onToggle={(event) => setRootsOpen(event.currentTarget.open)}
          >
            <summary>{t('runtimeAccess.restrict')}</summary>
            <label className="block space-y-2 text-normal">
              <span>{t('runtimeAccess.roots')}</span>
              <SettingsTextarea
                value={rootsText ?? current.settings.file_roots.join('\n')}
                disabled={busy}
                monospace
                onChange={(value) => {
                  setRootsText(value);
                  patch({
                    file_roots: value
                      .split('\n')
                      .map((root) => root.trim())
                      .filter(Boolean),
                  });
                }}
              />
            </label>
            <p>{t('runtimeAccess.osAuthority')}</p>
            <p>{t('runtimeAccess.permissions')}</p>
            <p>{t('runtimeAccess.revoke')}</p>
          </details>
          <SettingsSaveBar
            show={!!draft}
            saving={busy}
            onSave={() => void save()}
            onDiscard={() => {
              setDraft(null);
              setRootsText(null);
              setError('');
              void query.refetch();
            }}
          />
        </div>
      )}
    </SettingsCard>
  );
}
