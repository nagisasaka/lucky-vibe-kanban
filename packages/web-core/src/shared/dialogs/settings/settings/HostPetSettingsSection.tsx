import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { PetSettings } from '@/features/pets/SidebarPet';
import {
  SettingsCard,
  SettingsInput,
  SettingsSaveBar,
} from './SettingsComponents';
import {
  useSettingsHost,
  useSettingsMachineClient,
} from './SettingsHostContext';
import { useSettingsDirty } from './SettingsDirtyContext';

export function HostPetSettingsSection() {
  const host = useSettingsHost();
  return <HostPetSettingsContent key={host.selectedHostId ?? 'none'} />;
}
function HostPetSettingsContent() {
  const { t, i18n } = useTranslation('common');
  const client = useSettingsMachineClient();
  const host = useSettingsHost();
  const cache = useQueryClient();
  const { setDirty } = useSettingsDirty();
  const [draft, setDraft] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const key = [
    ...(client?.queryScopeKey ?? ['machine', 'none']),
    'host-display-name',
  ];
  const query = useQuery({
    queryKey: key,
    enabled: !!client,
    retry: false,
    queryFn: async () => {
      const data = await client!.getHostDisplayName();
      if (
        !data ||
        !Object.prototype.hasOwnProperty.call(data, 'host_display_name') ||
        !(
          data.host_display_name === null ||
          typeof data.host_display_name === 'string'
        )
      )
        throw new Error('Unsupported');
      return data;
    },
  });
  const environment = useQuery({
    queryKey: [
      ...(client?.queryScopeKey ?? ['machine', 'none']),
      'environment-info',
    ],
    enabled: !!client,
    retry: false,
    queryFn: () => client!.getHostEnvironment(),
  });
  const value = draft ?? query.data?.host_display_name ?? '';
  const normalized = value.trim() || null;
  const dirty =
    draft !== null && normalized !== (query.data?.host_display_name ?? null);
  const valid =
    Array.from(value.trim()).length <= 80 &&
    !/[\p{Cc}\u202a-\u202e\u2066-\u2069]/u.test(value.trim());
  useEffect(() => {
    setDirty('host-pet', dirty);
    return () => setDirty('host-pet', false);
  }, [dirty, setDirty]);
  async function save() {
    if (!client || !host.canEdit || !query.data || !dirty || !valid) return;
    setBusy(true);
    setError('');
    setSaved(false);
    try {
      const result = await client.saveHostDisplayName(normalized);
      cache.setQueryData(key, result);
      setDraft(null);
      setSaved(true);
      await cache.invalidateQueries({ queryKey: ['environment-info'] });
      await cache.invalidateQueries({
        queryKey: [...client.queryScopeKey, 'environment-info'],
      });
    } catch {
      setError(t('hostPet.saveError'));
    } finally {
      setBusy(false);
    }
  }
  const info = environment.data;
  return (
    <div className="space-y-6" data-testid="host-pet-settings">
      <SettingsCard
        title={t('hostPet.name')}
        description={t('hostPet.description')}
      >
        <div className="space-y-4">
          <label
            htmlFor="host-display-name"
            className="block text-sm font-medium text-high"
          >
            {t('hostPet.name')}
          </label>
          <SettingsInput
            id="host-display-name"
            value={value}
            onChange={(v) => {
              setDraft(v);
              setSaved(false);
              setError('');
            }}
            placeholder={info?.hostname ?? ''}
            disabled={!host.canEdit || busy || !query.data}
            error={!valid}
          />
          <p className="text-sm text-low">{t('hostPet.fallback')}</p>
          {!valid && (
            <p role="alert" className="text-sm text-error">
              {t('hostPet.validation')}
            </p>
          )}
          {query.isError && (
            <p role="alert" className="text-sm text-error">
              {t('hostPet.unsupported')}
            </p>
          )}
          {error && (
            <p role="alert" className="text-sm text-error">
              {error}
            </p>
          )}
          {saved && (
            <p role="status" className="text-sm text-success">
              {t('hostPet.saved')}
            </p>
          )}
          <dl className="grid min-w-0 gap-3 text-sm sm:grid-cols-2 [&_dt]:text-low [&_dd]:break-all [&_dd]:text-high">
            <div>
              <dt>{t('hostPet.hostname')}</dt>
              <dd>{info?.hostname ?? '—'}</dd>
            </div>
            <div>
              <dt>{t('utilityPanel.scope')}</dt>
              <dd>
                {info
                  ? t(
                      info.container
                        ? 'utilityPanel.container'
                        : 'utilityPanel.scopeUnknown'
                    )
                  : '—'}
              </dd>
            </div>
            <div>
              <dt>{t('environment.sessions')}</dt>
              <dd>{info?.running_sessions ?? '—'}</dd>
            </div>
            <div>
              <dt>{t('environment.lastCheck')}</dt>
              <dd>
                {info?.observed_at
                  ? new Date(info.observed_at).toLocaleString(i18n.language)
                  : '—'}
              </dd>
            </div>
            <div>
              <dt>{t('utilityPanel.uptime')}</dt>
              <dd>{info?.uptime_seconds ?? '—'}</dd>
            </div>
          </dl>
          {environment.isError && (
            <p role="status" className="text-sm text-low">
              {t('environment.unavailable')}
            </p>
          )}
        </div>
      </SettingsCard>
      <SettingsSaveBar
        show={dirty}
        saving={busy}
        saveDisabled={!valid || !query.data || !host.canEdit}
        onSave={() => {
          void save();
        }}
        onDiscard={() => {
          setDraft(null);
          setError('');
          setSaved(false);
        }}
      />
      <SettingsCard
        title={t('pets.settings')}
        description={t('hostPet.petDescription')}
      >
        <div className="min-w-0">
          <PetSettings />
        </div>
      </SettingsCard>
    </div>
  );
}
