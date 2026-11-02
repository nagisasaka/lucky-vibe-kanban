import type { TFunction } from 'i18next';
import type { WorkflowNodeData, WorkflowNodeKind } from './workflowGraph';

export type WorkflowNodeFieldType =
  | 'text'
  | 'textarea'
  | 'select'
  | 'number'
  | 'condition_branches'
  | 'arena_attempts';

export interface WorkflowNodeFieldOption {
  label: (t: TFunction<'common'>) => string;
  value: string;
}

export interface WorkflowNodeFieldSchema {
  key: keyof WorkflowNodeData;
  type: WorkflowNodeFieldType;
  label: (t: TFunction<'common'>) => string;
  options?: WorkflowNodeFieldOption[];
  rows?: number;
}

export interface WorkflowNodeSchema {
  type: WorkflowNodeKind;
  fields: WorkflowNodeFieldSchema[];
}

export const WORKFLOW_NODE_SCHEMAS: Record<
  WorkflowNodeKind,
  WorkflowNodeSchema
> = {
  start: {
    type: 'start',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
    ],
  },
  end: {
    type: 'end',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
    ],
  },
  agent: {
    type: 'agent',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
      {
        key: 'role_template_id',
        type: 'text',
        label: (t) => t('workflow.fields.role_template_id'),
      },
      {
        key: 'prompt_template',
        type: 'textarea',
        label: (t) => t('workflow.fields.prompt_template'),
        rows: 4,
      },
    ],
  },
  condition: {
    type: 'condition',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
      {
        key: 'routing_mode',
        type: 'select',
        label: (t) => t('workflow.fields.routing_mode'),
        options: [
          {
            label: (t) => t('workflow.fieldOptions.routing_mode.single'),
            value: 'single',
          },
          {
            label: (t) => t('workflow.fieldOptions.routing_mode.multi'),
            value: 'multi',
          },
        ],
      },
      {
        key: 'branches',
        type: 'condition_branches',
        label: (t) => t('workflow.fields.branches'),
      },
    ],
  },
  human_gate: {
    type: 'human_gate',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
      {
        key: 'prompt_to_human',
        type: 'textarea',
        label: (t) => t('workflow.fields.prompt_to_human'),
        rows: 3,
      },
      {
        key: 'required_action',
        type: 'select',
        label: (t) => t('workflow.fields.required_action'),
        options: [
          {
            label: (t) => t('workflow.fieldOptions.required_action.approve'),
            value: 'approve',
          },
          {
            label: (t) =>
              t('workflow.fieldOptions.required_action.approve_or_reject'),
            value: 'approve_or_reject',
          },
        ],
      },
    ],
  },
  transform: {
    type: 'transform',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
      {
        key: 'mode',
        type: 'select',
        label: (t) => t('workflow.fields.mode'),
        options: [
          {
            label: (t) => t('workflow.fieldOptions.mode.template'),
            value: 'template',
          },
          {
            label: (t) => t('workflow.fieldOptions.mode.regex_extract'),
            value: 'regex_extract',
          },
          {
            label: (t) => t('workflow.fieldOptions.mode.truncate'),
            value: 'truncate',
          },
        ],
      },
      {
        key: 'template',
        type: 'textarea',
        label: (t) => t('workflow.fields.template'),
        rows: 3,
      },
      { key: 'regex', type: 'text', label: (t) => t('workflow.fields.regex') },
      {
        key: 'max_chars',
        type: 'number',
        label: (t) => t('workflow.fields.max_chars'),
      },
    ],
  },
  arena: {
    type: 'arena',

    fields: [
      {
        key: 'display_name',
        type: 'text',
        label: (t) => t('workflow.fields.display_name'),
      },
      {
        key: 'prompt_template',
        type: 'textarea',
        label: (t) => t('workflow.fields.prompt_template'),
        rows: 4,
      },
      {
        key: 'attempts',
        type: 'arena_attempts',
        label: (t) => t('workflow.fields.attempts'),
      },
    ],
  },
};

export function getWorkflowNodeSchema(
  type: WorkflowNodeKind
): WorkflowNodeSchema {
  return WORKFLOW_NODE_SCHEMAS[type];
}

export function getWorkflowNodeSchemaFields(
  type: WorkflowNodeKind
): WorkflowNodeFieldSchema[] {
  return getWorkflowNodeSchema(type).fields;
}

export type ResolvedWorkflowNodeFieldSchema = Omit<
  WorkflowNodeFieldSchema,
  'label' | 'options'
> & {
  label: string;
  options?: Array<Omit<WorkflowNodeFieldOption, 'label'> & { label: string }>;
};

export function resolveWorkflowNodeField(
  field: WorkflowNodeFieldSchema,
  t: TFunction<'common'>
): ResolvedWorkflowNodeFieldSchema {
  const { options, ...rest } = field;
  return {
    ...rest,
    label: field.label(t),
    ...(options
      ? {
          options: options.map((option) => ({
            ...option,
            label: option.label(t),
          })),
        }
      : {}),
  };
}
