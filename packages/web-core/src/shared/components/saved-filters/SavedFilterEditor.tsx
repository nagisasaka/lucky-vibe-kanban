import { useTranslation } from 'react-i18next';
import type {
  MachineFilter,
  SavedFilterDefinition,
  SavedFilterPeriod,
  SavedFilterRelationKind,
} from 'shared/remote-types';
import { SettingsInput } from '@/shared/dialogs/settings/settings/SettingsComponents';

export interface FilterOption {
  id: string;
  label: string;
}
export interface SavedFilterOptions {
  statuses: FilterOption[];
  assignees: FilterOption[];
  tags: FilterOption[];
  issues: FilterOption[];
}
const control =
  'w-full min-w-0 rounded-sm border border-border bg-primary p-base text-normal focus-visible:ring-2 focus-visible:ring-brand';
function Choices({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: FilterOption[];
  value: string[];
  onChange: (value: string[]) => void;
}) {
  return (
    <fieldset className="min-w-0 space-y-half">
      <legend className="font-medium text-high">{label}</legend>
      <div className="flex max-h-40 flex-wrap gap-base overflow-auto rounded-sm border border-border p-base">
        {options.map((option) => (
          <label key={option.id} className="flex items-center gap-half">
            <input
              type="checkbox"
              checked={value.includes(option.id)}
              onChange={(e) =>
                onChange(
                  e.target.checked
                    ? [...value, option.id]
                    : value.filter((id) => id !== option.id)
                )
              }
            />
            {option.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}
function Period({
  label,
  value,
  onChange,
}: {
  label: string;
  value: SavedFilterPeriod | null;
  onChange: (value: SavedFilterPeriod | null) => void;
}) {
  const { t } = useTranslation('settings');
  return (
    <fieldset className="min-w-0 space-y-base">
      <legend className="font-medium text-high">{label}</legend>
      <select
        className={control}
        aria-label={label}
        value={value?.kind ?? ''}
        onChange={(e) =>
          onChange(
            e.target.value === 'absolute'
              ? { kind: 'absolute', start: null, end: null }
              : e.target.value === 'relative'
                ? { kind: 'relative', seconds: 604800 }
                : null
          )
        }
      >
        <option value="">{t('savedFilters.periodAny')}</option>
        <option value="absolute">{t('savedFilters.absolute')}</option>
        <option value="relative">{t('savedFilters.relative')}</option>
      </select>
      {value?.kind === 'absolute' && (
        <>
          <p className="text-low">{t('savedFilters.absoluteHelp')}</p>
          {(['start', 'end'] as const).map((bound) => (
            <label key={bound} className="block">
              {t(`savedFilters.${bound}`)}
              <input
                className={control}
                type="datetime-local"
                value={value[bound] ? toLocalTime(value[bound]) : ''}
                onChange={(e) =>
                  onChange({
                    ...value,
                    [bound]: e.target.value
                      ? new Date(e.target.value).toISOString()
                      : null,
                  })
                }
              />
            </label>
          ))}
        </>
      )}
      {value?.kind === 'relative' && (
        <label className="block">
          {t('savedFilters.seconds')}
          <input
            className={control}
            type="number"
            min="1"
            max="4294967295"
            required
            value={value.seconds}
            onChange={(e) =>
              onChange({ kind: 'relative', seconds: Number(e.target.value) })
            }
          />
        </label>
      )}
    </fieldset>
  );
}
function toLocalTime(value: string) {
  const date = new Date(value);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}
export function SavedFilterEditor({
  value,
  onChange,
  options,
  disabled = false,
}: {
  value: SavedFilterDefinition;
  onChange: (value: SavedFilterDefinition) => void;
  options: SavedFilterOptions;
  disabled?: boolean;
}) {
  const { t } = useTranslation('settings');
  const machine = value.machine;
  const changeMachine = (patch: Partial<MachineFilter>) =>
    onChange({ ...value, machine: { ...machine, ...patch } });
  const relations: SavedFilterRelationKind[] = [
    'parent',
    'children',
    'descendants',
    'related',
    'blocking',
    'blocked_by',
  ];
  return (
    <fieldset disabled={disabled} className="min-w-0 space-y-double">
      <label className="block space-y-half">
        <span>{t('savedFilters.name')}</span>
        <SettingsInput
          value={value.name}
          onChange={(name) => onChange({ ...value, name })}
          placeholder={t('savedFilters.name')}
        />
      </label>
      <p className="text-low">{t('savedFilters.combination')}</p>
      <label className="block space-y-half">
        <span>{t('savedFilters.search')}</span>
        <SettingsInput
          value={machine.search ?? ''}
          onChange={(search) => changeMachine({ search: search || null })}
        />
        <span className="block text-low">{t('savedFilters.searchHelp')}</span>
      </label>
      <label className="block space-y-half">
        <span>{t('savedFilters.simpleId')}</span>
        <SettingsInput
          value={machine.simple_id ?? ''}
          onChange={(simple_id) =>
            changeMachine({ simple_id: simple_id || null })
          }
        />
        <span className="block text-low">{t('savedFilters.simpleIdHelp')}</span>
      </label>
      <div className="grid min-w-0 gap-double md:grid-cols-2">
        <Choices
          label={t('savedFilters.statuses')}
          options={options.statuses}
          value={machine.status_ids}
          onChange={(status_ids) => changeMachine({ status_ids })}
        />
        <Choices
          label={t('savedFilters.priorities')}
          options={(['urgent', 'high', 'medium', 'low'] as const).map((id) => ({
            id,
            label: t(`common:kanban.priorityLabels.${id}`, id),
          }))}
          value={machine.priorities}
          onChange={(priorities) =>
            changeMachine({
              priorities: priorities as MachineFilter['priorities'],
            })
          }
        />
        <div className="space-y-base">
          <Choices
            label={t('savedFilters.assignees')}
            options={options.assignees.map((option) => ({
              ...option,
              label: `${option.label} (${option.id})`,
            }))}
            value={machine.assignee_ids}
            onChange={(assignee_ids) => changeMachine({ assignee_ids })}
          />
          <label className="flex items-center gap-half">
            <input
              type="checkbox"
              checked={machine.include_unassigned ?? false}
              onChange={(e) =>
                changeMachine({ include_unassigned: e.target.checked })
              }
            />
            {t('savedFilters.includeUnassigned')}
          </label>
          <p className="text-low">{t('savedFilters.assigneesHelp')}</p>
        </div>
        <div className="space-y-base">
          <Choices
            label={t('savedFilters.tags')}
            options={options.tags}
            value={machine.tag_ids}
            onChange={(tag_ids) => changeMachine({ tag_ids })}
          />
          <label className="block">
            {t('savedFilters.tagMode')}
            <select
              className={control}
              value={machine.tag_mode}
              onChange={(e) =>
                changeMachine({
                  tag_mode: e.target.value as MachineFilter['tag_mode'],
                })
              }
            >
              <option value="any">{t('savedFilters.any')}</option>
              <option value="all">{t('savedFilters.all')}</option>
            </select>
          </label>
        </div>
        <Period
          label={t('savedFilters.created')}
          value={machine.created_at}
          onChange={(created_at) => changeMachine({ created_at })}
        />
        <Period
          label={t('savedFilters.updated')}
          value={machine.updated_at}
          onChange={(updated_at) => changeMachine({ updated_at })}
        />
      </div>
      <fieldset className="space-y-base">
        <legend>{t('savedFilters.relation')}</legend>
        <select
          className={control}
          aria-label={t('savedFilters.relation')}
          value={machine.relation?.kind ?? ''}
          onChange={(e) =>
            changeMachine({
              relation: e.target.value
                ? {
                    kind: e.target.value as SavedFilterRelationKind,
                    target_issue_id: machine.relation?.target_issue_id ?? '',
                  }
                : null,
            })
          }
        >
          <option value="">{t('savedFilters.periodAny')}</option>
          {relations.map((kind) => (
            <option key={kind} value={kind}>
              {
                (
                  {
                    parent: t('savedFilters.relations.parent'),
                    children: t('savedFilters.relations.children'),
                    descendants: t('savedFilters.relations.descendants'),
                    related: t('savedFilters.relations.related'),
                    blocking: t('savedFilters.relations.blocking'),
                    blocked_by: t('savedFilters.relations.blocked_by'),
                  } as const
                )[kind]
              }
            </option>
          ))}
        </select>
        {machine.relation && (
          <label className="block">
            {t('savedFilters.target')}
            <select
              className={control}
              required
              aria-label={t('savedFilters.target')}
              value={machine.relation.target_issue_id}
              onChange={(e) =>
                changeMachine({
                  relation: {
                    ...machine.relation!,
                    target_issue_id: e.target.value,
                  },
                })
              }
            >
              <option value="">{t('savedFilters.chooseIssue')}</option>
              {options.issues.map((issue) => (
                <option key={issue.id} value={issue.id}>
                  {issue.label}
                </option>
              ))}
            </select>
          </label>
        )}
      </fieldset>
      <label className="block">
        {t('savedFilters.archive')}
        <select
          className={control}
          value={machine.archived}
          onChange={(e) =>
            changeMachine({
              archived: e.target.value as MachineFilter['archived'],
            })
          }
        >
          {(['active', 'archived', 'all'] as const).map((scope) => (
            <option key={scope} value={scope}>
              {t(`savedFilters.archiveScopes.${scope}`)}
            </option>
          ))}
        </select>
      </label>
      <label className="block space-y-half">
        <span>{t('savedFilters.naturalLanguage')}</span>
        <textarea
          className={control}
          rows={3}
          value={value.natural_language ?? ''}
          onChange={(e) =>
            onChange({ ...value, natural_language: e.target.value || null })
          }
        />
      </label>
      <label className="block space-y-half">
        <span>{t('savedFilters.context')}</span>
        <textarea
          className={control}
          rows={3}
          value={value.context ?? ''}
          onChange={(e) =>
            onChange({ ...value, context: e.target.value || null })
          }
        />
        <span className="block text-low">{t('savedFilters.contextHelp')}</span>
      </label>
    </fieldset>
  );
}
