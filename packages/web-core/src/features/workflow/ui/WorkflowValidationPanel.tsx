import type { TFunction } from 'i18next';
import {
  WORKFLOW_GRAPH_VERSION,
  type WorkflowConditionBranch,
  type WorkflowGraph,
} from '../model/workflowGraph';
import { AlertCircle, CheckCircle2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { BaseCodingAgent } from 'shared/types';

export interface ValidationIssue {
  type: 'error' | 'warning';
  nodeId?: string;
  code:
    | 'unsupportedVersion'
    | 'startCount'
    | 'endRequired'
    | 'duplicateId'
    | 'selfEdge'
    | 'missingSource'
    | 'missingTarget'
    | 'cycle'
    | 'unreachable'
    | 'duplicateOutgoing'
    | 'routerRequired'
    | 'missingBranch'
    | 'branchTargetRequired'
    | 'duplicateBranch'
    | 'staleBranch'
    | 'emptyCondition';
  values?: { node?: string; target?: string; count?: number };
}

export function workflowValidationMessage(
  issue: ValidationIssue,
  t: TFunction<'common'>,
  locale: string
): string {
  const values = {
    ...issue.values,
    formattedCount: new Intl.NumberFormat(locale).format(
      issue.values?.count ?? 0
    ),
  };
  switch (issue.code) {
    case 'unsupportedVersion':
      return t('workflow.validationMessages.unsupportedVersion', values);
    case 'startCount':
      return t('workflow.validationMessages.startCount', values);
    case 'endRequired':
      return t('workflow.validationMessages.endRequired', values);
    case 'duplicateId':
      return t('workflow.validationMessages.duplicateId', values);
    case 'selfEdge':
      return t('workflow.validationMessages.selfEdge', values);
    case 'missingSource':
      return t('workflow.validationMessages.missingSource', values);
    case 'missingTarget':
      return t('workflow.validationMessages.missingTarget', values);
    case 'cycle':
      return t('workflow.validationMessages.cycle', values);
    case 'unreachable':
      return t('workflow.validationMessages.unreachable', values);
    case 'duplicateOutgoing':
      return t('workflow.validationMessages.duplicateOutgoing', values);
    case 'routerRequired':
      return t('workflow.validationMessages.routerRequired', values);
    case 'missingBranch':
      return t('workflow.validationMessages.missingBranch', values);
    case 'branchTargetRequired':
      return t('workflow.validationMessages.branchTargetRequired', values);
    case 'duplicateBranch':
      return t('workflow.validationMessages.duplicateBranch', values);
    case 'staleBranch':
      return t('workflow.validationMessages.staleBranch', values);
    case 'emptyCondition':
      return t('workflow.validationMessages.emptyCondition', values);
  }
}

export interface WorkflowValidationPanelProps {
  graph: WorkflowGraph | null;
}

export interface WorkflowValidationOptions {
  includeRunReadiness?: boolean;
}

export function validateWorkflowGraph(
  graph: WorkflowGraph | null,
  options: WorkflowValidationOptions = {}
): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  if (!graph) return issues;
  const includeRunReadiness = options.includeRunReadiness ?? true;

  if (graph.version < 1 || graph.version > WORKFLOW_GRAPH_VERSION) {
    issues.push({ type: 'error', code: 'unsupportedVersion' });
  }

  const nodes = graph.nodes;
  const edges = graph.edges;

  const startNodes = nodes.filter((n) => n.type === 'start');
  if (startNodes.length !== 1) {
    issues.push({
      type: 'error',
      code: 'startCount',
      values: { count: startNodes.length },
    });
  }

  const endNodes = nodes.filter((n) => n.type === 'end');
  if (endNodes.length === 0) {
    issues.push({
      type: 'error',
      code: 'endRequired',
    });
  }

  const nodeIds = new Set<string>();
  for (const node of nodes) {
    if (nodeIds.has(node.id)) {
      issues.push({
        type: 'error',
        code: 'duplicateId',
        values: { node: node.id },
      });
    }
    nodeIds.add(node.id);
  }

  for (const edge of edges) {
    if (edge.source === edge.target) {
      issues.push({
        type: 'error',
        nodeId: edge.source,
        code: 'selfEdge',
        values: { node: edge.source },
      });
    }
    if (!nodeIds.has(edge.source)) {
      issues.push({
        type: 'error',
        nodeId: edge.source,
        code: 'missingSource',
        values: { node: edge.source },
      });
    }
    if (!nodeIds.has(edge.target)) {
      issues.push({
        type: 'error',
        nodeId: edge.target,
        code: 'missingTarget',
        values: { node: edge.target },
      });
    }
  }

  validateConditionStructure(graph, issues);

  const adjacency = buildAdjacency(graph);
  const cycleNodeId = findCycleNode(adjacency);
  if (cycleNodeId) {
    issues.push({
      type: 'error',
      nodeId: cycleNodeId,
      code: 'cycle',
      values: { node: cycleNodeId },
    });
  }

  if (startNodes.length === 1) {
    const reachable = findReachableNodes(startNodes[0].id, adjacency);
    for (const node of nodes) {
      if (node.type !== 'start' && !reachable.has(node.id)) {
        issues.push({
          type: 'error',
          nodeId: node.id,
          code: 'unreachable',
          values: { node: node.id },
        });
      }
    }
  }

  if (includeRunReadiness) {
    validateConditionRunReadiness(graph, issues);
  }

  return issues;
}

function getConditionOutgoingTargets(
  graph: WorkflowGraph,
  conditionNodeId: string
): string[] {
  return graph.edges
    .filter((edge) => edge.source === conditionNodeId)
    .map((edge) => edge.target);
}

function validateConditionStructure(
  graph: WorkflowGraph,
  issues: ValidationIssue[]
) {
  for (const node of graph.nodes) {
    if (node.type !== 'condition') continue;

    const seenTargets = new Set<string>();
    for (const target of getConditionOutgoingTargets(graph, node.id)) {
      if (seenTargets.has(target)) {
        issues.push({
          type: 'error',
          nodeId: node.id,
          code: 'duplicateOutgoing',
          values: { node: node.id, target },
        });
      }
      seenTargets.add(target);
    }
  }
}

function validateConditionRunReadiness(
  graph: WorkflowGraph,
  issues: ValidationIssue[]
) {
  const conditionNodes = graph.nodes.filter(
    (node) => node.type === 'condition'
  );
  if (conditionNodes.length === 0) return;

  const hasRouterConfig = hasRouterExecutorConfig(graph.router_executor_config);
  if (!hasRouterConfig) {
    issues.push({
      type: 'error',
      code: 'routerRequired',
    });
  }

  for (const node of conditionNodes) {
    const outgoingTargets = new Set(
      getConditionOutgoingTargets(graph, node.id)
    );
    const branchTargets = new Set<string>();
    const branches = node.data.branches ?? [];

    for (const branch of branches) {
      validateConditionBranch(
        node.id,
        branch,
        outgoingTargets,
        branchTargets,
        issues
      );
    }

    for (const target of outgoingTargets) {
      if (!branchTargets.has(target)) {
        issues.push({
          type: 'error',
          nodeId: node.id,
          code: 'missingBranch',
          values: { node: node.id, target },
        });
      }
    }
  }
}

function hasRouterExecutorConfig(config: unknown): boolean {
  if (!config || typeof config !== 'object') return false;
  const executor = (config as { executor?: unknown }).executor;
  if (typeof executor !== 'string') return false;
  const normalized = executor.trim().replaceAll('-', '_').toUpperCase();
  return Object.values(BaseCodingAgent).includes(normalized as BaseCodingAgent);
}

function validateConditionBranch(
  nodeId: string,
  branch: WorkflowConditionBranch,
  outgoingTargets: Set<string>,
  branchTargets: Set<string>,
  issues: ValidationIssue[]
) {
  const targetNodeId = branch.target_node_id;
  if (!targetNodeId) {
    issues.push({
      type: 'error',
      nodeId,
      code: 'branchTargetRequired',
      values: { node: nodeId },
    });
    return;
  }

  if (branchTargets.has(targetNodeId)) {
    issues.push({
      type: 'error',
      nodeId,
      code: 'duplicateBranch',
      values: { node: nodeId, target: targetNodeId },
    });
  }
  branchTargets.add(targetNodeId);

  if (!outgoingTargets.has(targetNodeId)) {
    issues.push({
      type: 'error',
      nodeId,
      code: 'staleBranch',
      values: { node: nodeId, target: targetNodeId },
    });
  }

  if (!(branch.condition ?? '').trim()) {
    issues.push({
      type: 'error',
      nodeId,
      code: 'emptyCondition',
      values: { node: nodeId, target: targetNodeId },
    });
  }
}

function buildAdjacency(graph: WorkflowGraph): Map<string, string[]> {
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const adjacency = new Map<string, string[]>();
  for (const node of graph.nodes) {
    adjacency.set(node.id, []);
  }

  for (const edge of graph.edges) {
    if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) {
      continue;
    }
    adjacency.get(edge.source)?.push(edge.target);
  }

  return adjacency;
}

function findCycleNode(adjacency: Map<string, string[]>): string | null {
  const states = new Map<string, 'visiting' | 'visited'>();

  const visit = (nodeId: string): string | null => {
    const state = states.get(nodeId);
    if (state === 'visiting') return nodeId;
    if (state === 'visited') return null;

    states.set(nodeId, 'visiting');
    for (const target of adjacency.get(nodeId) ?? []) {
      const cycleNodeId = visit(target);
      if (cycleNodeId) return cycleNodeId;
    }
    states.set(nodeId, 'visited');
    return null;
  };

  for (const nodeId of adjacency.keys()) {
    const cycleNodeId = visit(nodeId);
    if (cycleNodeId) return cycleNodeId;
  }

  return null;
}

function findReachableNodes(
  startNodeId: string,
  adjacency: Map<string, string[]>
): Set<string> {
  const reachable = new Set<string>();
  const queue = [startNodeId];

  while (queue.length > 0) {
    const nodeId = queue.shift();
    if (!nodeId || reachable.has(nodeId)) continue;
    reachable.add(nodeId);
    queue.push(...(adjacency.get(nodeId) ?? []));
  }

  return reachable;
}

export function WorkflowValidationPanel({
  graph,
}: WorkflowValidationPanelProps) {
  const { t, i18n } = useTranslation('common');
  const issues = validateWorkflowGraph(graph);

  return (
    <div className="max-h-48 overflow-y-auto border-t border-secondary bg-panel p-4 text-sm">
      <h3 className="mb-2 font-semibold text-high">
        {t('workflow.editor.validation')}
      </h3>
      {issues.length === 0 ? (
        <div className="flex items-center gap-2 text-success">
          <CheckCircle2 className="h-4 w-4" />
          <span>{t('workflow.editor.graphValid')}</span>
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {issues.map((issue, i) => (
            <li
              key={i}
              className={`flex items-start gap-2 ${
                issue.type === 'error' ? 'text-error' : 'text-brand'
              }`}
            >
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
              <span>
                {workflowValidationMessage(
                  issue,
                  t,
                  i18n.resolvedLanguage || i18n.language
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
