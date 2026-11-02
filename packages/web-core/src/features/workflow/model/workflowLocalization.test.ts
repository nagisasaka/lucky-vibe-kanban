import { getWorkflowAgentDisplay } from './workflowAgentDisplay';
import { describe, expect, it } from 'vitest';
import i18n from '@/i18n/config';
import { WORKFLOW_GRAPH_VERSION, type WorkflowGraph } from './workflowGraph';
import { createDefaultNodeData } from './workflowNodeCatalog';
import {
  getWorkflowNodeSchemaFields,
  resolveWorkflowNodeField,
} from './workflowNodeSchemas';
import {
  validateWorkflowGraph,
  workflowValidationMessage,
} from '../ui/WorkflowValidationPanel';

describe('workflow localization boundaries', () => {
  it('keeps validation semantic and preserves raw node IDs across locales', () => {
    const graph: WorkflowGraph = {
      version: WORKFLOW_GRAPH_VERSION,
      nodes: [
        { id: 'raw-start', type: 'start', data: {} },
        {
          id: '原文-{{node}}',
          type: 'agent',
          data: { prompt_template: 'Raw prompt {{input}}' },
        },
      ],
      edges: [],
    };
    const original = JSON.stringify(graph);
    const issues = validateWorkflowGraph(graph);
    const issue = issues.find((item) => item.code === 'unreachable')!;
    expect(issue).toEqual({
      type: 'error',
      nodeId: '原文-{{node}}',
      code: 'unreachable',
      values: { node: '原文-{{node}}' },
    });
    expect(
      workflowValidationMessage(issue, i18n.getFixedT('en', 'common'), 'en')
    ).toBe('Unreachable node: 原文-{{node}}');
    expect(
      workflowValidationMessage(issue, i18n.getFixedT('ja', 'common'), 'ja')
    ).toBe('到達できないノードです: 原文-{{node}}');
    expect(JSON.stringify(graph)).toBe(original);
    expect(validateWorkflowGraph(graph)).toEqual(issues);
  });

  it('retains version and structural requirements and localizes count values', () => {
    const issues = validateWorkflowGraph({
      version: WORKFLOW_GRAPH_VERSION + 1,
      nodes: [],
      edges: [],
    });
    expect(issues.map((issue) => issue.code)).toEqual([
      'unsupportedVersion',
      'startCount',
      'endRequired',
    ]);
    expect(
      workflowValidationMessage(issues[1], i18n.getFixedT('ja', 'common'), 'ja')
    ).toBe('開始ノードは1つ必要です。現在は0個あります');
  });

  it('resolves presentation without changing field keys, option values or default data', () => {
    const defaults = JSON.stringify(createDefaultNodeData('condition'));
    const fields = getWorkflowNodeSchemaFields('condition');
    const en = fields.map((field) =>
      resolveWorkflowNodeField(field, i18n.getFixedT('en', 'common'))
    );
    const ja = fields.map((field) =>
      resolveWorkflowNodeField(field, i18n.getFixedT('ja', 'common'))
    );
    expect(ja.map((field) => field.label)).toEqual([
      '表示名',
      '分岐モード',
      '分岐',
    ]);
    expect(en[1].options?.map((option) => option.value)).toEqual([
      'single',
      'multi',
    ]);
    expect(ja[1].options?.map((option) => option.value)).toEqual([
      'single',
      'multi',
    ]);
    expect(ja[1].options?.map((option) => option.label)).toEqual([
      '1つの分岐',
      '複数の分岐',
    ]);
    expect(ja.map((field) => field.key)).toEqual(en.map((field) => field.key));
    expect(JSON.stringify(createDefaultNodeData('condition'))).toBe(defaults);
  });
});

it('translates default agent/model labels while preserving configured identities', () => {
  const t = i18n.getFixedT('ja', 'common');
  expect(getWorkflowAgentDisplay({}, t).detailLabel).toBe(
    '既定のエージェント / 既定のモデル'
  );
  const configured = getWorkflowAgentDisplay(
    {
      executor_config: {
        executor: 'CODEX',
        variant: null,
        model_id: 'Raw model 原文',
        reasoning_id: 'raw-reasoning',
      },
    },
    t
  );
  expect(configured.modelLabel).toBe('Raw model 原文');
  expect(configured.reasoningLabel).toBe('raw-reasoning');
});
