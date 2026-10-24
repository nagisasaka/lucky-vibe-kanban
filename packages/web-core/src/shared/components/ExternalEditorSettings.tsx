import type { EditorConfig } from 'shared/types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { useTranslation } from 'react-i18next';

export function ExternalEditorSettings({
  value,
  onChange,
}: {
  value: EditorConfig;
  onChange: (value: EditorConfig) => void;
}) {
  const { t } = useTranslation('settings');
  const entries = value.entries ?? [];
  const update = (
    index: number,
    field: 'name' | 'uri_template',
    text: string
  ) =>
    onChange({
      entries: entries.map((entry, i) =>
        i === index ? { ...entry, [field]: text } : entry
      ),
    });
  const move = (index: number, offset: number) => {
    const next = [...entries];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    onChange({ entries: next });
  };
  return (
    <div className="space-y-base">
      <p className="text-sm text-low">{t('externalEditors.description')}</p>
      <p className="text-sm text-low">{t('externalEditors.variables')}</p>
      <p className="text-sm text-low">{t('externalEditors.migration')}</p>
      {entries.map((entry, index) => (
        <div
          key={index}
          className="space-y-half rounded border border-border p-base"
        >
          <label className="block text-sm text-normal">
            {t('externalEditors.name')}
            <input
              className="w-full rounded border border-border bg-panel p-half text-high focus:ring-1 focus:ring-brand"
              value={entry.name}
              maxLength={80}
              onChange={(e) => update(index, 'name', e.target.value)}
            />
          </label>
          <label className="block text-sm text-normal">
            {t('externalEditors.template')}
            <input
              className="w-full rounded border border-border bg-panel p-half font-ibm-plex-mono text-high focus:ring-1 focus:ring-brand"
              value={entry.uri_template}
              maxLength={8192}
              onChange={(e) => update(index, 'uri_template', e.target.value)}
            />
          </label>
          <div className="flex gap-half">
            <PrimaryButton
              className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:opacity-50"
              type="button"
              variant="tertiary"
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              {t('externalEditors.up')}
            </PrimaryButton>
            <PrimaryButton
              className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:opacity-50"
              type="button"
              variant="tertiary"
              disabled={index === entries.length - 1}
              onClick={() => move(index, 1)}
            >
              {t('externalEditors.down')}
            </PrimaryButton>
            <PrimaryButton
              className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:opacity-50"
              type="button"
              variant="tertiary"
              onClick={() =>
                onChange({ entries: entries.filter((_, i) => i !== index) })
              }
            >
              {t('externalEditors.remove')}
            </PrimaryButton>
          </div>
        </div>
      ))}
      <PrimaryButton
        className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand disabled:opacity-50"
        type="button"
        variant="tertiary"
        onClick={() =>
          onChange({ entries: [...entries, { name: '', uri_template: '' }] })
        }
      >
        {t('externalEditors.add')}
      </PrimaryButton>
    </div>
  );
}
