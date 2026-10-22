import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { Grant, RuntimeAccessView } from 'shared/types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { runtimeAccessSettings } from '@/shared/lib/runtimeAccessApi';
import {
  SettingsCard,
  SettingsCheckbox,
  SettingsInput,
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
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const { setDirty } = useSettingsDirty();
  const current = query.error ? undefined : (draft ?? query.data);
  useEffect(() => {
    setDirty('mcp-integration', !!draft);
    return () => setDirty('mcp-integration', false);
  }, [draft, setDirty]);
  function patch(index: number, change: Partial<Grant>) {
    if (!current) return;
    setDraft({
      ...current,
      settings: {
        ...current.settings,
        grants: current.settings.grants.map((g, i) =>
          i === index ? { ...g, ...change } : g
        ),
      },
    });
  }
  async function save(recover = false) {
    if (!current) return;
    setBusy(true);
    setError('');
    try {
      await runtimeAccessSettings({
        revision: current.revision,
        settings: recover ? { enabled: false, grants: [] } : current.settings,
      });
      await query.refetch();
      setDraft(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  const field = (label: string, value: string, change: (v: string) => void) => (
    <label className="block space-y-2 text-sm text-normal">
      <span>{label}</span>
      <SettingsInput value={value} onChange={change} disabled={busy} />
    </label>
  );
  return (
    <SettingsCard
      title={t('runtimeAccess.title')}
      description={t('runtimeAccess.description')}
    >
      <details className="text-sm text-low">
        <summary>{t('runtimeAccess.help')}</summary>
        <p>{t('runtimeAccess.permissions')}</p>
        <p>{t('runtimeAccess.osAuthority')}</p>
        <p>{t('runtimeAccess.revoke')}</p>
      </details>
      {(error || query.error) && (
        <p role="alert" className="text-sm text-error">
          {error || t('runtimeAccess.denied')}
        </p>
      )}
      {query.isLoading && <p role="status">{t('runtimeAccess.loading')}</p>}
      {current?.legacy_error && (
        <div role="alert" className="space-y-3 text-sm text-error">
          <p>{t('runtimeAccess.invalidLegacy')}</p>
          <p>{current.legacy_error.reason}</p>
          <PrimaryButton
            value={t('runtimeAccess.recover')}
            disabled={busy}
            onClick={() => void save(true)}
          />
        </div>
      )}
      {current && !current.legacy_error && (
        <div className="space-y-4">
          <p className="text-sm text-low">
            {t(`runtimeAccess.source.${current.source}`)}
          </p>
          <SettingsCheckbox
            id="runtime-enabled"
            label={t('runtimeAccess.enabled')}
            checked={current.settings.enabled}
            disabled={busy}
            onChange={(enabled) =>
              setDraft({
                ...current,
                settings: { ...current.settings, enabled },
              })
            }
          />
          {current.settings.grants.map((grant, index) => (
            <fieldset
              key={grant.id}
              className="space-y-3 border border-border rounded-sm p-base"
              disabled={busy}
            >
              <legend className="text-normal">
                {grant.display_name || grant.id}
              </legend>
              {field(
                t('runtimeAccess.name'),
                grant.display_name,
                (display_name) => patch(index, { display_name })
              )}
              <label className="block space-y-2 text-sm text-normal">
                <span>{t('runtimeAccess.project')}</span>
                <select
                  className="w-full bg-secondary border border-border rounded-sm p-half text-high"
                  value={grant.project_id}
                  onChange={(e) => patch(index, { project_id: e.target.value })}
                >
                  <option value="">—</option>
                  {current.projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
              {field(t('runtimeAccess.root'), grant.root, (root) =>
                patch(index, { root })
              )}
              <SettingsCheckbox
                id={`execute-${grant.id}`}
                label={t('runtimeAccess.execute')}
                checked={grant.allow_uncoordinated_execution}
                disabled={busy}
                onChange={(allow_uncoordinated_execution) =>
                  patch(index, { allow_uncoordinated_execution })
                }
              />
              <PrimaryButton
                value={t('runtimeAccess.remove')}
                variant="tertiary"
                disabled={busy}
                onClick={() =>
                  setDraft({
                    ...current,
                    settings: {
                      ...current.settings,
                      grants: current.settings.grants.filter(
                        (_, i) => i !== index
                      ),
                    },
                  })
                }
              />
            </fieldset>
          ))}
          <PrimaryButton
            value={t('runtimeAccess.add')}
            disabled={busy || current.settings.grants.length >= 64}
            onClick={() =>
              setDraft({
                ...current,
                settings: {
                  ...current.settings,
                  grants: [
                    ...current.settings.grants,
                    {
                      id: crypto.randomUUID(),
                      display_name: '',
                      project_id: '',
                      root: '',
                      allow_uncoordinated_execution: false,
                    },
                  ],
                },
              })
            }
          />
          <SettingsSaveBar
            show={!!draft}
            saving={busy}
            onSave={() => void save()}
            onDiscard={() => {
              setDraft(null);
              setError('');
              void query.refetch();
            }}
          />
        </div>
      )}
    </SettingsCard>
  );
}
