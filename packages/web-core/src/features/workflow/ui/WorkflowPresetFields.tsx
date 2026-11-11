import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  Preset,
  PresetRequest,
  PresetDefaults,
  Stage,
} from 'shared/types';
import { workflowPresetsApi } from '@/shared/lib/workflowApi';

export const presetTranslationKeys = {
  implement_review: 'workflow.presets.implement_review',
  research_synthesize: 'workflow.presets.research_synthesize',
  inventory_reduce: 'workflow.presets.inventory_reduce',
} as const satisfies Record<Preset, string>;

export function initialPreset(preset: Preset): PresetRequest {
  return {
    preset,
    scope: '',
    additions: '',
    project_additions: '',
    project_revision: 0,
    perspectives: ['Facts and contracts', 'Reduction and maintenance'],
    stages: {},
  };
}

const field =
  'w-full rounded border border-secondary bg-primary p-half text-sm text-normal';

export function StageFields({
  name,
  value,
  onChange,
  taskParameters = false,
}: {
  name: string;
  value: Stage;
  onChange(value: Stage): void;
  taskParameters?: boolean;
}) {
  const { t } = useTranslation('common');
  return (
    <fieldset className="flex flex-wrap items-center gap-half rounded border border-secondary p-half">
      <legend className="text-xs text-low">{name}</legend>
      <select
        aria-label={`${name} engine`}
        className={field}
        value={value.engine}
        onChange={(e) =>
          onChange({
            ...value,
            engine: e.target.value as Stage['engine'],
            model: null,
            variant: null,
          })
        }
      >
        <option value="CODEX">{t('environment.codex')}</option>
        <option value="CLAUDE_CODE">{t('environment.claude')}</option>
      </select>
      <input
        aria-label={`${name} model`}
        placeholder={t('workflow.presets.model')}
        className={field}
        value={value.model ?? ''}
        onChange={(e) => onChange({ ...value, model: e.target.value || null })}
      />
      <input
        aria-label={`${name} profile`}
        placeholder={t('workflow.presets.profile')}
        className={field}
        value={value.variant ?? ''}
        onChange={(e) =>
          onChange({ ...value, variant: e.target.value || null })
        }
      />
      {taskParameters && (
        <>
          <input
            aria-label={`${name} scope`}
            placeholder={t('workflow.presets.scope')}
            className={field}
            value={value.scope ?? ''}
            onChange={(e) =>
              onChange({ ...value, scope: e.target.value || undefined })
            }
          />
          <textarea
            aria-label={`${name} additions`}
            placeholder={t('workflow.presets.additions')}
            className={field}
            value={value.additions ?? ''}
            onChange={(e) =>
              onChange({ ...value, additions: e.target.value || undefined })
            }
          />
        </>
      )}
    </fieldset>
  );
}

export function WorkflowPresetFields({
  projectId,
  value,
  onChange,
  onReady,
}: {
  projectId: string;
  value: PresetRequest | null;
  onChange(value: PresetRequest | null): void;
  onReady(ready: boolean): void;
}) {
  const { t } = useTranslation('common');
  const [defaults, setDefaults] = useState<PresetDefaults[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setDefaults(null);
    setError(null);
    void workflowPresetsApi
      .defaults(projectId)
      .then((v) => {
        if (active) setDefaults(v);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  const ready =
    !value ||
    (!!defaults &&
      value.perspectives.length > 0 &&
      value.perspectives.length <= 8 &&
      value.perspectives.every((v) => !!v.trim()));
  useEffect(() => onReady(ready), [ready, onReady]);
  const stages = !value
    ? []
    : value.preset === 'implement_review'
      ? ['implement', 'review']
      : value.preset === 'inventory_reduce'
        ? ['inventory', 'reduce']
        : [
            ...value.perspectives.map((_, i) => `research-${i + 1}`),
            'synthesize',
          ];

  return (
    <section className="space-y-base rounded border border-secondary p-base">
      <label className="block text-sm text-high">
        {t('workflow.presets.method')}
        <select
          className={field}
          value={value?.preset ?? 'single'}
          onChange={(e) => {
            if (e.target.value === 'single') {
              onChange(null);
              return;
            }
            const preset = e.target.value as Preset;
            const d = defaults?.find((v) => v.preset === preset);
            onChange({
              ...initialPreset(preset),
              project_additions: d?.additions ?? '',
              project_revision: d?.revision ?? 0,
            });
          }}
        >
          <option value="single">{t('workflow.presets.single')}</option>
          {Object.entries(presetTranslationKeys).map(([id, key]) => (
            <option key={id} value={id} disabled={!defaults}>
              {t(key)}
            </option>
          ))}
        </select>
      </label>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      {value && (
        <>
          <p className="text-xs text-low">
            {t('workflow.presets.stopContract')}
          </p>
          <label className="block text-sm">
            {t('workflow.presets.scope')}
            <input
              className={field}
              value={value.scope}
              onChange={(e) => onChange({ ...value, scope: e.target.value })}
            />
          </label>
          {value.preset === 'research_synthesize' && (
            <label className="block text-sm">
              {t('workflow.presets.perspectives')}
              <textarea
                className={field}
                rows={3}
                value={value.perspectives.join('\n')}
                onChange={(e) => {
                  const perspectives = e.target.value.split('\n');
                  const allowed = new Set([
                    ...perspectives.map((_, i) => `research-${i + 1}`),
                    'synthesize',
                  ]);
                  onChange({
                    ...value,
                    perspectives,
                    stages: Object.fromEntries(
                      Object.entries(value.stages).filter(([key]) =>
                        allowed.has(key)
                      )
                    ),
                  });
                }}
              />
            </label>
          )}
          {stages.map((name) => (
            <StageFields
              key={name}
              name={name}
              taskParameters
              value={
                value.stages[name] ?? {
                  engine: 'CODEX',
                  model: null,
                  variant: null,
                }
              }
              onChange={(stage) =>
                onChange({
                  ...value,
                  stages: { ...value.stages, [name]: stage },
                })
              }
            />
          ))}
          <label className="block text-sm">
            {t('workflow.presets.additions')}
            <textarea
              className={field}
              rows={3}
              value={value.additions}
              onChange={(e) =>
                onChange({ ...value, additions: e.target.value })
              }
            />
          </label>
          <details>
            <summary className="text-xs text-low">
              {t('workflow.presets.projectSource')} {value.project_revision}
            </summary>
            <pre className="whitespace-pre-wrap text-xs">
              {value.project_additions || '—'}
            </pre>
          </details>
        </>
      )}
    </section>
  );
}
