import type { WorkflowNodeKind, WorkflowNodeData } from './workflowGraph';

export interface CatalogEntry {
  type: WorkflowNodeKind;
  defaultData: WorkflowNodeData;
}

export interface WorkflowNodeCatalogSection {
  id: 'execution' | 'control' | 'structure';
  label: (t: (key: string) => string) => string;
  entries: CatalogEntry[];
}

export const WORKFLOW_NODE_CATALOG: CatalogEntry[] = [
  {
    type: 'start',
    defaultData: { display_name: 'Start' },
  },
  {
    type: 'end',
    defaultData: { display_name: 'End' },
  },
  {
    type: 'agent',
    defaultData: {
      display_name: 'Agent',
      role_template_id: 'custom',
      prompt_template: '',
    },
  },
  {
    type: 'condition',
    defaultData: {
      display_name: 'Condition',
      routing_mode: 'single',
      branches: [],
    },
  },
  {
    type: 'human_gate',
    defaultData: {
      display_name: 'Human Gate',
      required_action: 'approve_or_reject',
    },
  },
  {
    type: 'transform',
    defaultData: {
      display_name: 'Transform',
      mode: 'template',
      template: '{{input}}',
    },
  },
  {
    type: 'arena',
    defaultData: {
      display_name: 'Arena',
      promote_strategy: 'manual',
      apply_strategy: 'diff_apply',
      attempts: [
        {
          id: 'attempt-a',
          display_name: 'Attempt A',
          role_template_id: 'custom',
          prompt_template: '',
        },
        {
          id: 'attempt-b',
          display_name: 'Attempt B',
          role_template_id: 'custom',
          prompt_template: '',
        },
      ],
    },
  },
];

export function isWorkflowNodeAuthorable(kind: WorkflowNodeKind): boolean {
  return kind !== 'start' && kind !== 'end' && kind !== 'arena';
}

export function createDefaultNodeData(
  kind: WorkflowNodeKind
): WorkflowNodeData {
  const entry = WORKFLOW_NODE_CATALOG.find((e) => e.type === kind);
  if (!entry) {
    return { display_name: kind };
  }
  // Deep clone to prevent mutating catalog defaults
  return JSON.parse(JSON.stringify(entry.defaultData)) as WorkflowNodeData;
}

export function getWorkflowNodeCatalogSections(): WorkflowNodeCatalogSection[] {
  const entryByType = new Map(
    WORKFLOW_NODE_CATALOG.filter((entry) =>
      isWorkflowNodeAuthorable(entry.type)
    ).map((entry) => [entry.type, entry])
  );
  const sections: Array<{
    id: WorkflowNodeCatalogSection['id'];
    label: (t: (key: string) => string) => string;
    types: WorkflowNodeKind[];
  }> = [
    {
      id: 'execution',
      label: (t) => t('workflow.editor.sections.execution'),
      types: ['agent'],
    },
    {
      id: 'control',
      label: (t) => t('workflow.editor.sections.control'),
      types: ['condition', 'human_gate'],
    },
  ];

  return sections
    .map((section) => ({
      id: section.id,
      label: section.label,
      entries: section.types
        .map((type) => entryByType.get(type))
        .filter((entry): entry is CatalogEntry => Boolean(entry)),
    }))
    .filter((section) => section.entries.length > 0);
}
