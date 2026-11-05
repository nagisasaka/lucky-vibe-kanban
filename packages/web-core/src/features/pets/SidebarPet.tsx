import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { JsonValue } from 'shared/types';
import { attachmentsApi } from '@/shared/lib/api';
import { persistPetPreferences } from '@/shared/lib/petPreferencesPersistence';
import { useUiPreferencesStore } from '@/shared/stores/useUiPreferencesStore';
import { usePetBehavior } from './usePetBehavior';
import { PetSprite } from './PetSprite';
import { PetError, PET_STATES, type PetState } from './format';
import {
  DEFAULT_PET,
  importPet,
  restorePreferences,
  type PetPreferences,
} from './library';

const SIDEBAR_PET_SIZE = 80;

export function SidebarPet({
  state = 'idle',
}: {
  state?: 'idle' | 'running' | 'failed';
}) {
  return <PetContent liveState={state} />;
}
export function PetSettings() {
  return <PetContent settings />;
}
function PetContent({
  settings = false,
  liveState = 'idle',
}: {
  settings?: boolean;
  liveState?: 'idle' | 'running' | 'failed';
}) {
  const behavior = usePetBehavior(liveState, !settings);
  const { t, i18n } = useTranslation('common');
  const raw = useUiPreferencesStore((s) => s.petPreferences);
  const save = useUiPreferencesStore((s) => s.setPetPreferences);
  const preferences = restorePreferences(raw);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [fileName, setFileName] = useState('');
  const [imported, setImported] = useState('');
  const feedbackId = useId();
  const [failedSource, setFailedSource] = useState('');
  const [preview, setPreview] = useState<PetState>('idle');
  const selected = preferences.pets.find((p) => p.key === preferences.selected);
  const source = selected
    ? attachmentsApi.getAttachmentUrl(selected.attachmentId)
    : '/pets/lvk-rabbit/spritesheet.webp';
  const src =
    failedSource === source ? '/pets/lvk-rabbit/spritesheet.webp' : source;
  const isFallback = selected !== undefined && failedSource === source;
  const name = isFallback
    ? 'LVK Rabbit'
    : (selected?.manifest.displayName ?? 'LVK Rabbit');
  const commit = async (next: PetPreferences) => {
    const value = JSON.parse(JSON.stringify(next)) as JsonValue;
    await persistPetPreferences(value);
    save(value);
  };
  const update = async (next: PetPreferences) => {
    setFileName('');
    setImported('');
    setBusy(true);
    setError('');
    try {
      await commit(next);
    } catch {
      setError(t('pets.errors.storage'));
    } finally {
      setBusy(false);
    }
  };
  const upload = async (file: File) => {
    setFileName(file.name);
    setImported('');
    setBusy(true);
    setError('');
    try {
      if (preferences.pets.length >= 12)
        throw new PetError('collection_size', preferences.pets.length, 12);
      const pet = await importPet(file);
      const attachment = await attachmentsApi.upload(pet.image);
      // Read latest preferences after the asynchronous import; never lose a concurrent selection.
      const current = restorePreferences(
        useUiPreferencesStore.getState().petPreferences
      );
      if (current.pets.length >= 12)
        throw new PetError('collection_size', current.pets.length, 12);
      const key = crypto.randomUUID();
      await commit({
        selected: key,
        hidden: false,
        pets: [
          ...current.pets,
          {
            key,
            attachmentId: attachment.id,
            manifest: pet.manifest,
            originalManifest: pet.originalManifest,
            licenseText: pet.licenseText,
          },
        ],
      });
      setFailedSource('');
      setPreview('idle');
      setImported(pet.manifest.displayName);
    } catch (e) {
      setError(
        t(`pets.errors.${e instanceof PetError ? e.code : 'storage'}`, {
          actual:
            e instanceof PetError
              ? e.actual?.toLocaleString(i18n.language)
              : undefined,
          limit:
            e instanceof PetError
              ? e.limit?.toLocaleString(i18n.language)
              : undefined,
        })
      );
    } finally {
      setBusy(false);
    }
  };
  const sprite = (
    <div
      className="shrink-0"
      style={{ width: SIDEBAR_PET_SIZE }}
      data-testid={settings ? 'pet-settings-preview' : 'sidebar-pet'}
    >
      {!preferences.hidden && (
        <PetSprite
          key={src}
          src={src}
          name={name}
          size={SIDEBAR_PET_SIZE}
          state={settings ? preview : behavior}
          version={
            isFallback ? 1 : (selected?.manifest.spriteVersionNumber ?? 1)
          }
          onError={() => {
            setFailedSource(source);
            setError(t('pets.errors.storage'));
          }}
        />
      )}
    </div>
  );
  if (!settings) return sprite;
  return (
    <div className="space-y-base text-sm" data-testid="pet-settings">
      {sprite}
      <div className="grid min-w-0 gap-4 sm:grid-cols-2 [&>p]:sm:col-span-2 [&>details]:sm:col-span-2">
        <label className="block">
          {t('pets.select')}
          <select
            className="block w-full bg-secondary border rounded-sm p-half"
            value={preferences.selected}
            disabled={busy}
            onChange={(e) => {
              void update({ ...preferences, selected: e.target.value });
              setPreview('idle');
              setError('');
            }}
          >
            <option value={DEFAULT_PET}>LVK Rabbit</option>
            {preferences.pets.map((p) => (
              <option key={p.key} value={p.key}>
                {p.manifest.displayName}
              </option>
            ))}
          </select>
        </label>
        <label className="flex gap-half items-center">
          <input
            type="checkbox"
            disabled={busy}
            checked={!preferences.hidden}
            onChange={(e) =>
              void update({ ...preferences, hidden: !e.target.checked })
            }
          />
          {t('pets.show')}
        </label>
        <label className="block">
          {t('pets.import')}
          <input
            type="file"
            accept=".zip,application/zip"
            aria-describedby={feedbackId}
            disabled={busy}
            className="block w-full"
            onChange={(e) => {
              const file = e.target.files?.[0];
              e.target.value = '';
              if (file) void upload(file);
            }}
          />
        </label>
        <p
          id={feedbackId}
          role="status"
          aria-live="polite"
          aria-atomic="true"
          className={`break-words ${error ? 'text-error' : 'text-normal'}`}
        >
          {fileName && <span className="block text-low">{fileName}</span>}
          {busy
            ? t('pets.importing')
            : error || (imported ? t('pets.imported', { name: imported }) : '')}
        </p>
        <p>{t('pets.scope')}</p>
        <a
          href="https://petdex.dev/"
          target="_blank"
          rel="noopener noreferrer"
          className="underline"
        >
          {t('pets.gallery')}
        </a>
        <label className="block">
          {t('pets.preview')}
          <select
            className="block w-full bg-secondary border rounded-sm p-half"
            value={preview}
            onChange={(e) => setPreview(e.target.value as PetState)}
          >
            {PET_STATES.map((s) => (
              <option key={s.name} value={s.name}>
                {t(`pets.states.${s.name}`)}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="underline"
          disabled={busy}
          onClick={() => {
            void update({
              ...preferences,
              selected: DEFAULT_PET,
              hidden: false,
            });
            setPreview('idle');
          }}
        >
          {t('pets.reset')}
        </button>
        {selected && (
          <button
            type="button"
            className="block underline"
            disabled={busy}
            onClick={() => {
              void update({
                ...preferences,
                selected: DEFAULT_PET,
                pets: preferences.pets.filter((p) => p.key !== selected.key),
              });
              setPreview('idle');
            }}
          >
            {t('pets.remove')}
          </button>
        )}
        {isFallback && (
          <p>{t('pets.fallback', { name: selected.manifest.displayName })}</p>
        )}
        {selected && (
          <details>
            <summary>
              {t(isFallback ? 'pets.retainedAttribution' : 'pets.attribution')}
            </summary>
            <p className="whitespace-pre-wrap break-words">
              {selected.manifest.author} {selected.manifest.license}
            </p>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words">
              {selected.originalManifest}
              {'\n'}
              {selected.licenseText}
            </pre>
          </details>
        )}
        {(!selected || isFallback) && (
          <p>
            LVK Rabbit — Masato Fujino ·{' '}
            <a
              className="underline"
              href="https://github.com/nagisasaka/lvk-rabbit/tree/v1.1.0"
              target="_blank"
              rel="noopener noreferrer"
            >
              v1.1.0
            </a>{' '}
            ·{' '}
            <a
              className="underline"
              href="https://creativecommons.org/licenses/by/4.0/"
              target="_blank"
              rel="noopener noreferrer"
            >
              CC BY 4.0
            </a>{' '}
            · {t('pets.unmodified')}
          </p>
        )}
      </div>
    </div>
  );
}
