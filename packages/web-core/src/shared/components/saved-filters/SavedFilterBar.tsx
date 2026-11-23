import { useState, useMemo } from 'react';
import { SavedFilterStatus } from './SavedFilterStatus';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import {
  SavedFilterEditor,
  type SavedFilterOptions,
} from './SavedFilterEditor';
import { savedFilterHref } from '@/shared/lib/savedFilters';
import type { SavedFilterView } from '@/shared/hooks/useSavedFilter';

export function SavedFilterBar({
  view,
  count,
}: {
  view: SavedFilterView;
  count: number;
}) {
  const { t } = useTranslation('settings');
  const project = useProjectContext();
  const org = useOrgContext();
  const [expanded, setExpanded] = useState(false);
  const options: SavedFilterOptions = useMemo(
    () => ({
      statuses: project.statuses.map((s) => ({ id: s.id, label: s.name })),
      tags: project.tags.map((s) => ({ id: s.id, label: s.name })),
      issues: project.issues.map((s) => ({
        id: s.id,
        label: `${s.simple_id} ${s.title}`,
      })),
      assignees: [...org.membersWithProfilesById.values()].map((s) => ({
        id: s.user_id,
        label: s.first_name || s.username || s.user_id,
      })),
    }),
    [
      project.statuses,
      project.tags,
      project.issues,
      org.membersWithProfilesById,
    ]
  );
  return (
    <div className="min-w-0 space-y-base">
      <div className="flex flex-wrap items-center gap-base">
        <label className="flex min-w-0 max-w-full items-center gap-base">
          <span className="shrink-0">{t('savedFilters.select')}</span>
          <select
            aria-label={t('savedFilters.select')}
            className="min-w-0 max-w-72 rounded-sm border border-border bg-primary p-half"
            value={view.id ?? '__none__'}
            onChange={(e) =>
              view.select(
                e.target.value === '__none__' ? undefined : e.target.value
              )
            }
          >
            <option value="__none__">{t('savedFilters.none')}</option>
            {view.id !== undefined &&
              !view.list.data?.some((f) => f.id === view.id) && (
                <option value={view.id}>
                  {view.definition.data?.name ?? view.id}
                </option>
              )}
            {view.list.data?.map((filter) => (
              <option key={filter.id} value={filter.id}>
                {filter.name}
              </option>
            ))}
          </select>
        </label>
        <PrimaryButton
          variant="tertiary"
          value={t('savedFilters.manage')}
          onClick={() =>
            void SettingsDialog.show({
              initialSection: 'saved-filters',
              initialState: {
                projectId: project.projectId,
                organizationId: org.organizationId,
              },
            })
          }
        />
        {view.selected && (
          <>
            <PrimaryButton
              variant="tertiary"
              value={t('savedFilters.refresh')}
              onClick={view.refresh}
              disabled={view.evaluation.isFetching}
            />
            <a
              className="text-brand underline"
              href={savedFilterHref(project.projectId, view.id!)}
              target="_blank"
              rel="noreferrer"
            >
              {t('savedFilters.link')}
            </a>
          </>
        )}
      </div>
      {view.list.isError && (
        <p role="alert">
          {t('savedFilters.error')}{' '}
          <button
            className="underline"
            onClick={() => void view.list.refetch()}
          >
            {t('savedFilters.retry')}
          </button>
        </p>
      )}
      {view.selected && (
        <div className="space-y-base rounded-sm border border-border bg-secondary p-base">
          <p className="text-low">{t('savedFilters.override')}</p>
          <SavedFilterStatus view={view} count={count} />
          {view.definition.data && (
            <details
              onToggle={(event) => setExpanded(event.currentTarget.open)}
            >
              <summary className="cursor-pointer text-brand">
                {t('savedFilters.conditions')} · {view.definition.data.name}
              </summary>
              <div className="mt-base max-h-[50vh] overflow-auto">
                {expanded && (
                  <SavedFilterEditor
                    disabled
                    value={view.definition.data}
                    onChange={() => {}}
                    options={options}
                  />
                )}
              </div>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
