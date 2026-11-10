import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { workflowApi } from '@/shared/lib/workflowApi';
import { useWorkflowAttemptForWorkflow } from '@/shared/hooks/useWorkflowAttempts';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { Button } from '@vibe/ui/components/Button';

export interface WorkflowTemplateEditorPageProps {
  projectId: string;
  workflowId: string;
}

/** Historical URL retained as a definition viewer. */
export function WorkflowTemplateEditorPage({
  projectId,
  workflowId,
}: WorkflowTemplateEditorPageProps) {
  const { t } = useTranslation('common');
  const navigation = useAppNavigation();
  const { data: workflow, error } = useQuery({
    queryKey: ['workflow-history-definition', workflowId],
    queryFn: () => workflowApi.get(workflowId),
  });
  const { data: attempt } = useWorkflowAttemptForWorkflow(workflowId);

  return (
    <main className="h-full space-y-base overflow-auto p-double text-normal">
      <h1 className="text-xl text-high">
        {workflow?.name ?? t('workflow.presets.history')}
      </h1>
      <p>{t('workflow.presets.historyNotice')}</p>
      {attempt?.latest_run_id && (
        <Button
          onClick={() => navigation.goToProjectWorkflowRun(
            projectId,
            attempt.latest_run_id!
          )}
        >
          {t('workflow.presets.results')}
        </Button>
      )}
      {error && <p role="alert" className="text-error">{String(error)}</p>}
      <pre className="whitespace-pre-wrap rounded border border-secondary bg-panel p-base text-xs">
        {workflow?.graph_json}
      </pre>
    </main>
  );
}
