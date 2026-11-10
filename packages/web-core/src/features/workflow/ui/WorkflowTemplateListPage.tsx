import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useWorkflowTemplates } from '@/shared/hooks/useWorkflowTemplates';
import { workflowPresetsApi } from '@/shared/lib/workflowApi';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { Button } from '@vibe/ui/components/Button';
import type { PresetDefaults } from 'shared/types';
import { presetNames } from './WorkflowPresetFields';

export interface WorkflowTemplateListPageProps {
  projectId: string;
}

function DefaultsEditor({ projectId, value }: {
  projectId: string;
  value: PresetDefaults;
}) {
  const { t } = useTranslation('common');
  const [text, setText] = useState(value.additions);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const query = useQueryClient();

  return (
    <section className="space-y-half rounded border border-secondary p-base">
      <h2 className="text-high">
        {t(`workflow.presets.${value.preset}`, presetNames[value.preset])}
      </h2>
      <label className="block text-sm">
        {t('workflow.presets.projectAdditions', 'Project additions (fixed at each launch)')}
        <textarea
          className="w-full rounded border border-secondary bg-primary p-half"
          rows={3}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </label>
      <Button
        disabled={saving || text === value.additions}
        onClick={async () => {
          setSaving(true);
          setError(null);
          try {
            await workflowPresetsApi.save(projectId, { ...value, additions: text });
            await query.invalidateQueries({
              queryKey: ['workflow-preset-defaults', projectId],
            });
          } catch (e) {
            setError(String(e));
          } finally {
            setSaving(false);
          }
        }}
      >
        {t('buttons.save', 'Save')}
      </Button>
      {error && <p role="alert" className="text-error">{error}</p>}
    </section>
  );
}

export function WorkflowTemplateListPage({ projectId }: WorkflowTemplateListPageProps) {
  const { t } = useTranslation('common');
  const { data: defaults, error } = useQuery({
    queryKey: ['workflow-preset-defaults', projectId],
    queryFn: () => workflowPresetsApi.defaults(projectId),
  });
  const { data: history } = useWorkflowTemplates(projectId);
  const navigation = useAppNavigation();

  return (
    <main className="h-full space-y-base overflow-auto p-double text-normal">
      <h1 className="text-xl text-high">{t('workflow.presets.title', 'Workflow presets')}</h1>
      <p>
        {t('workflow.presets.startAtIssue', 'Choose single agent or a preset in the Issue start dialog. Presets return their reports and stop.')}
      </p>
      {error && <p role="alert" className="text-error">{String(error)}</p>}
      {defaults?.map((value) => (
        <DefaultsEditor
          key={`${value.preset}-${value.revision}`}
          projectId={projectId}
          value={value}
        />
      ))}
      <details>
        <summary>{t('workflow.presets.history', 'Historical definitions')}</summary>
        {history?.workflows.map((value) => (
          <div key={value.id}>
            <Button
              variant="outline"
              onClick={() => navigation.goToProjectWorkflowEdit(projectId, value.id)}
            >
              {value.name}
            </Button>
          </div>
        ))}
      </details>
    </main>
  );
}
