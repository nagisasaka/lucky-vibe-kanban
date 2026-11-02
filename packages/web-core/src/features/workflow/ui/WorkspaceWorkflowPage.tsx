import { useTranslation } from 'react-i18next';
import { useWorkspaceRepositoryWorkflow } from '@/shared/hooks/useWorkspaceRepositoryWorkflow';
import { WorkflowRunPage } from './WorkflowRunPage';

export function WorkspaceWorkflowPage({
  workspaceId,
}: {
  workspaceId: string;
}) {
  const { t } = useTranslation('common');
  const {
    data: run,
    isPending,
    error,
    refetch,
  } = useWorkspaceRepositoryWorkflow(workspaceId, null);

  if (isPending) {
    return (
      <p className="p-base text-normal" role="status">
        {t('workflow.repository.loading')}
      </p>
    );
  }
  if (error || !run) {
    return (
      <div className="space-y-base p-base">
        <p className="text-error" role="alert">
          {error
            ? t('workflow.repository.unavailable')
            : t('workflow.repository.missing')}
        </p>
        <button className="text-brand underline" onClick={() => void refetch()}>
          {t('workflow.repository.reload')}
        </button>
        <a
          className="block text-brand underline"
          href={`/workspaces/${workspaceId}`}
        >
          {t('workspaces.open')}
        </a>
      </div>
    );
  }
  return <WorkflowRunPage key={run.id} runId={run.id} />;
}
