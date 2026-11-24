import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { SavedFiltersManager } from '@/shared/components/saved-filters/SavedFiltersManager';
import { useSettingsDirty } from './SettingsDirtyContext';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';

export function SavedFiltersSettingsSection({
  initialState,
}: {
  initialState?: { organizationId?: string; projectId?: string };
}) {
  const { t } = useTranslation('settings');
  const orgs = useUserOrganizations();
  const [orgSelection, setOrgSelection] = useState(
    initialState?.organizationId
  );
  const orgId = orgSelection ?? orgs.data?.organizations[0]?.id ?? null;
  const projects = useOrganizationProjects(orgId);
  const [projectId, setProjectId] = useState(initialState?.projectId ?? '');
  const { getIsDirty, clearAll } = useSettingsDirty();
  async function change(id: string, organization = false) {
    if (
      getIsDirty() &&
      (await ConfirmDialog.show({
        title: t('savedFilters.unsaved'),
        message: t('savedFilters.unsaved'),
        confirmText: t('savedFilters.cancel'),
        cancelText: t('savedFilters.edit'),
      })) !== 'confirmed'
    )
      return;
    clearAll();
    setProjectId(organization ? '' : id);
    if (organization) setOrgSelection(id);
  }
  return (
    <div className="space-y-double">
      {orgs.isError || projects.isError ? (
        <p role="alert">{t('savedFilters.error')}</p>
      ) : null}
      <label className="block">
        {t('savedFilters.organization')}
        <select
          className="w-full rounded-sm border border-border bg-primary p-base"
          value={orgId ?? ''}
          onChange={(e) => void change(e.target.value, true)}
        >
          {orgs.data?.organizations.map((org) => (
            <option key={org.id} value={org.id}>
              {org.name}
            </option>
          ))}
        </select>
      </label>
      <label className="block">
        {t('savedFilters.project')}
        <select
          className="w-full rounded-sm border border-border bg-primary p-base"
          value={projectId}
          onChange={(e) => void change(e.target.value)}
        >
          <option value="">{t('savedFilters.chooseProject')}</option>
          {projects.data.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
      </label>
      {orgId && projects.data.some((project) => project.id === projectId) && (
        <OrgProvider organizationId={orgId}>
          <ProjectProvider key={projectId} projectId={projectId}>
            <SavedFiltersManager />
          </ProjectProvider>
        </OrgProvider>
      )}
    </div>
  );
}
