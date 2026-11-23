import { useEffect, useMemo, useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { SavedFilter, SavedFilterDefinition } from 'shared/remote-types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import { SettingsCard } from '@/shared/dialogs/settings/settings/SettingsComponents';
import { useSettingsDirty } from '@/shared/dialogs/settings/settings/SettingsDirtyContext';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import { SavedFilterEditor } from './SavedFilterEditor';
import { emptyDefinition, savedFilterHref } from '@/shared/lib/savedFilters';
import { savedFiltersApi, savedFilterKeys } from '@/shared/lib/savedFiltersApi';

export function SavedFiltersManager() {
  const { t } = useTranslation('settings');
  const project = useProjectContext();
  const { membersWithProfilesById } = useOrgContext();
  const { setDirty } = useSettingsDirty();
  const client = useQueryClient();
  const list = useQuery({
    queryKey: savedFilterKeys.list(project.projectId),
    queryFn: ({ signal }) => savedFiltersApi.list(project.projectId, signal),
  });
  const [editing, setEditing] = useState<SavedFilter | 'new' | null>(null);
  const [draft, setDraft] = useState<SavedFilterDefinition>(emptyDefinition);
  const [initial, setInitial] =
    useState<SavedFilterDefinition>(emptyDefinition);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingRequests = useRef(new Map<string, string>());
  function requestId(intent: unknown) {
    const key = JSON.stringify(intent);
    let id = pendingRequests.current.get(key);
    if (!id) {
      id = crypto.randomUUID();
      pendingRequests.current.set(key, id);
    }
    return id;
  }
  const dirty =
    editing !== null && JSON.stringify(draft) !== JSON.stringify(initial);
  useEffect(() => {
    setDirty('saved-filters', dirty);
    return () => setDirty('saved-filters', false);
  }, [dirty, setDirty]);
  const options = useMemo(
    () => ({
      statuses: project.statuses.map((s) => ({ id: s.id, label: s.name })),
      tags: project.tags.map((tag) => ({ id: tag.id, label: tag.name })),
      assignees: [...membersWithProfilesById.values()].map((m) => ({
        id: m.user_id,
        label: m.first_name || m.username || m.user_id,
      })),
      issues: project.issues.map((i) => ({
        id: i.id,
        label: `${i.simple_id} ${i.title}`,
      })),
    }),
    [project.statuses, project.tags, project.issues, membersWithProfilesById]
  );
  async function canDiscard() {
    return (
      !dirty ||
      (await ConfirmDialog.show({
        title: t('savedFilters.unsaved'),
        message: t('savedFilters.unsaved'),
        confirmText: t('savedFilters.cancel'),
        cancelText: t('savedFilters.edit'),
      })) === 'confirmed'
    );
  }
  async function select(filter: SavedFilter | 'new' | null) {
    if (!(await canDiscard())) return;
    setError(null);
    setBusy(true);
    try {
      const current =
        filter && filter !== 'new'
          ? await savedFiltersApi.get(project.projectId, filter.id)
          : filter;
      const definition =
        current && current !== 'new'
          ? {
              name: current.name,
              machine: current.machine,
              natural_language: current.natural_language,
              context: current.context,
            }
          : emptyDefinition();
      setEditing(current);
      setInitial(definition);
      setDraft(definition);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function save() {
    if (
      !draft.name.trim() ||
      (draft.machine.relation && !draft.machine.relation.target_issue_id) ||
      [draft.machine.created_at, draft.machine.updated_at].some((p) =>
        p?.kind === 'relative'
          ? !Number.isInteger(p.seconds) || p.seconds < 1
          : p?.kind === 'absolute' &&
            ((!p.start && !p.end) || (p.start && p.end && p.start >= p.end))
      )
    ) {
      setError(t('savedFilters.invalid'));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result =
        editing && editing !== 'new'
          ? await savedFiltersApi.update(project.projectId, editing.id, {
              request_id: requestId([editing, draft]),
              expected_revision: editing.revision,
              definition: draft,
            })
          : await savedFiltersApi.create(project.projectId, {
              request_id: requestId([editing, draft]),
              definition: draft,
            });
      setEditing(result);
      const definition = {
        name: result.name,
        machine: result.machine,
        natural_language: result.natural_language,
        context: result.context,
      };
      setDraft(definition);
      setInitial(definition);
      pendingRequests.current.clear();
      await client.invalidateQueries({
        queryKey: savedFilterKeys.project(project.projectId),
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function remove(filter: SavedFilter) {
    if (
      (await ConfirmDialog.show({
        title: t('savedFilters.delete'),
        message: t('savedFilters.deleteConfirm'),
        confirmText: t('savedFilters.delete'),
        cancelText: t('savedFilters.cancel'),
        variant: 'destructive',
      })) !== 'confirmed'
    )
      return;
    setBusy(true);
    setError(null);
    try {
      await savedFiltersApi.remove(project.projectId, filter.id, {
        request_id: requestId(['delete', filter.id, filter.revision]),
        expected_revision: filter.revision,
      });
      setEditing(null);
      pendingRequests.current.clear();
      await client.invalidateQueries({
        queryKey: savedFilterKeys.project(project.projectId),
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="space-y-double">
      <SettingsCard
        title={t('savedFilters.title')}
        description={t('savedFilters.shared')}
        headerAction={
          <PrimaryButton
            disabled={busy}
            value={t('savedFilters.new')}
            onClick={() => void select('new')}
          />
        }
      >
        {list.isPending && <p role="status">{t('savedFilters.loading')}</p>}
        {list.isError && (
          <p role="alert">
            {t('savedFilters.error')}{' '}
            <button className="underline" onClick={() => void list.refetch()}>
              {t('savedFilters.retry')}
            </button>
          </p>
        )}
        {list.data?.length === 0 && <p>{t('savedFilters.empty')}</p>}
        <ul className="divide-y divide-border">
          {list.data?.map((filter) => (
            <li
              key={filter.id}
              className="flex flex-wrap items-center justify-between gap-base py-base"
            >
              <button
                className="min-w-0 break-words text-left text-high underline"
                disabled={busy}
                onClick={() => void select(filter)}
              >
                {filter.name}
              </button>
              <a
                className="shrink-0 text-brand underline"
                href={savedFilterHref(project.projectId, filter.id)}
              >
                {t('savedFilters.open')}
              </a>
            </li>
          ))}
        </ul>
      </SettingsCard>
      {error && (
        <p role="alert" className="text-error">
          {t('savedFilters.error')} {error}
        </p>
      )}
      {editing && (
        <SettingsCard
          title={t(
            editing === 'new' ? 'savedFilters.new' : 'savedFilters.edit'
          )}
        >
          <form
            className="space-y-double"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
          >
            <SavedFilterEditor
              value={draft}
              onChange={setDraft}
              options={options}
              disabled={busy || project.isLoading}
            />
            <div className="flex flex-wrap gap-base">
              <PrimaryButton
                type="submit"
                disabled={busy}
                value={t('savedFilters.save')}
              />
              <PrimaryButton
                type="button"
                variant="tertiary"
                disabled={busy}
                onClick={() => void select(null)}
                value={t('savedFilters.cancel')}
              />
              {editing !== 'new' && (
                <>
                  <PrimaryButton
                    type="button"
                    variant="tertiary"
                    disabled={busy}
                    value={t('savedFilters.delete')}
                    onClick={() => void remove(editing)}
                  />
                  <PrimaryButton
                    type="button"
                    variant="tertiary"
                    disabled={busy}
                    value={t('savedFilters.reload')}
                    onClick={() => void select(editing)}
                  />
                </>
              )}
            </div>
          </form>
        </SettingsCard>
      )}
    </div>
  );
}
