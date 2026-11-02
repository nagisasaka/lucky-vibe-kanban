import { getWorkflowRunStatusLabel } from '@/features/workflow/model/workflowRunView';
import { useTranslation } from 'react-i18next';
import { useWorkspaceRepositoryWorkflow } from '@/shared/hooks/useWorkspaceRepositoryWorkflow';

export function WorkspaceWorkflowLink({
  workspaceId,
  hostId,
}: {
  workspaceId: string;
  hostId: string | null;
}) {
  const { t } = useTranslation('common');
  const { data: run, error } = useWorkspaceRepositoryWorkflow(
    workspaceId,
    hostId
  );
  if (hostId !== null) return null;
  if (error) {
    return (
      <p className="p-half text-xs text-error" role="alert">
        {t('workflow.repository.unavailable')} {error.message}
      </p>
    );
  }
  if (!run) return null;
  return (
    <a
      href={`/workspaces/${workspaceId}/workflow`}
      className="block border-b border-secondary bg-panel p-half text-sm text-brand underline"
    >
      {t('workflow.repository.view', {
        status: getWorkflowRunStatusLabel(run.status, t),
      })}
    </a>
  );
}
