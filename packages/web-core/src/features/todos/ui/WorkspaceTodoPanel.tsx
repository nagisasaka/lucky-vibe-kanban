import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useTodoViewStore } from '../model/todoViewStore';
import { todoApi } from '../model/todoApi';
import { TodoPanel } from './TodoPanel';

export function WorkspaceTodoPanel({ workspaceId }: { workspaceId: string }) {
  const { t } = useTranslation('common');
  const org = useOrganizationStore((s) => s.selectedOrgId);
  const projects = useOrganizationProjects(org);
  const scope = useQuery({
    queryKey: ['todo-workspace-project', workspaceId],
    queryFn: () => todoApi.workspaceProject(workspaceId),
    refetchInterval: 5000,
  });
  const selected = useTodoViewStore((s) => s.workspaceProjects[workspaceId]);
  const select = useTodoViewStore((s) => s.setWorkspaceProject);
  const projectId = scope.data ?? selected;
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border p-base flex flex-wrap items-center gap-base">
        <h2 className="font-medium text-high">{t('todo.title')}</h2>
        {scope.data ? (
          <span className="text-sm text-low">
            {projects.data.find((p) => p.id === scope.data)?.name ?? scope.data}
          </span>
        ) : (
          <select
            aria-label={t('todo.project')}
            value={selected ?? ''}
            disabled={scope.isLoading || scope.isError}
            onChange={(event) => select(workspaceId, event.target.value)}
            className="min-w-0 max-w-full rounded-sm border border-border bg-primary p-half text-normal"
          >
            <option value="">{t('todo.chooseProject')}</option>
            {projects.data.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        )}
      </div>
      {scope.error || projects.error ? (
        <p role="alert" className="p-base text-error">
          {scope.error?.message ?? projects.error?.message}
        </p>
      ) : scope.isLoading ? (
        <p className="p-base text-low">{t('states.loading')}</p>
      ) : projectId ? (
        <TodoPanel key={projectId} projectId={projectId} />
      ) : (
        <p className="p-double text-low">{t('todo.chooseProject')}</p>
      )}
    </div>
  );
}
