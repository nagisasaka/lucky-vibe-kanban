import type { TFunction } from 'i18next';
import i18n from '@/i18n/config';
import type {
  WorkflowEdgeKind,
  WorkflowNodeData,
  WorkflowNodeKind,
} from './workflowGraph';

const NODE_KIND_I18N_KEYS = {
  start: 'workflow.nodes.start.label',
  end: 'workflow.nodes.end.label',
  agent: 'workflow.nodes.agent.label',
  condition: 'workflow.nodes.condition.label',
  human_gate: 'workflow.nodes.humanGate.label',
  transform: 'workflow.nodes.transform.label',
  arena: 'workflow.nodes.arena.label',
} as const satisfies Record<WorkflowNodeKind, string>;

const EDGE_LABEL_I18N_KEYS = {
  condition_branch: 'workflow.edges.condition',
  approval: 'workflow.edges.approve',
  rejection: 'workflow.edges.reject',
  arena_winner: 'workflow.edges.winner',
} as const satisfies Record<Exclude<WorkflowEdgeKind, 'default'>, string>;

export interface WorkflowNodeVisual {
  accentClass: string;
  iconClass: string;
  badgeClass: string;
}

export interface WorkflowEdgeVisual {
  label?: string;
  pathClass: string;
  chipClass: string;
}

export interface WorkflowNodeMetadataChip {
  label: string;
  value: string;
}

export interface WorkflowNodeRouteHint {
  label: string;
  tone: 'brand' | 'success' | 'warning' | 'danger';
}

const NODE_VISUALS: Record<WorkflowNodeKind, WorkflowNodeVisual> = {
  start: {
    accentClass: 'bg-emerald-500',
    iconClass: 'bg-emerald-500/10 text-emerald-300',
    badgeClass: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300',
  },
  end: {
    accentClass: 'bg-slate-500',
    iconClass: 'bg-slate-500/10 text-slate-300',
    badgeClass: 'border-slate-500/30 bg-slate-500/10 text-slate-300',
  },
  agent: {
    accentClass: 'bg-sky-500',
    iconClass: 'bg-sky-500/10 text-sky-300',
    badgeClass: 'border-sky-500/30 bg-sky-500/10 text-sky-300',
  },
  condition: {
    accentClass: 'bg-amber-500',
    iconClass: 'bg-amber-500/10 text-amber-300',
    badgeClass: 'border-amber-500/30 bg-amber-500/10 text-amber-300',
  },
  human_gate: {
    accentClass: 'bg-violet-500',
    iconClass: 'bg-violet-500/10 text-violet-300',
    badgeClass: 'border-violet-500/30 bg-violet-500/10 text-violet-300',
  },
  transform: {
    accentClass: 'bg-cyan-500',
    iconClass: 'bg-cyan-500/10 text-cyan-300',
    badgeClass: 'border-cyan-500/30 bg-cyan-500/10 text-cyan-300',
  },
  arena: {
    accentClass: 'bg-fuchsia-500',
    iconClass: 'bg-fuchsia-500/10 text-fuchsia-300',
    badgeClass: 'border-fuchsia-500/30 bg-fuchsia-500/10 text-fuchsia-300',
  },
};

const EDGE_VISUALS: Record<WorkflowEdgeKind, WorkflowEdgeVisual> = {
  default: {
    pathClass: 'stroke-low/60',
    chipClass: 'border-secondary bg-panel text-low',
  },
  condition_branch: {
    pathClass: 'stroke-amber-500',
    chipClass: 'border-amber-500/30 bg-amber-500/10 text-amber-700',
  },
  approval: {
    pathClass: 'stroke-emerald-500',
    chipClass: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-700',
  },
  rejection: {
    pathClass: 'stroke-rose-500',
    chipClass: 'border-rose-500/30 bg-rose-500/10 text-rose-700',
  },
  arena_winner: {
    pathClass: 'stroke-fuchsia-500',
    chipClass: 'border-fuchsia-500/30 bg-fuchsia-500/10 text-fuchsia-700',
  },
};

export interface WorkflowEdgeKindOption {
  value: WorkflowEdgeKind;
  label: string;
  description: string;
}

const EDGE_KINDS: WorkflowEdgeKind[] = [
  'default',
  'condition_branch',
  'approval',
  'rejection',
  'arena_winner',
];

export function getWorkflowNodeKindLabel(
  kind: WorkflowNodeKind,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): string {
  return t(NODE_KIND_I18N_KEYS[kind]);
}

export function getWorkflowNodeSummary(
  kind: WorkflowNodeKind,
  data: WorkflowNodeData,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): string {
  switch (kind) {
    case 'start':
      return t('workflow.nodes.start.summary');
    case 'end':
      return t('workflow.nodes.end.summary');
    case 'agent':
      return t('workflow.nodes.agent.summary', {
        role: String(data.role_template_id || 'custom'),
      });
    case 'condition':
      return t('workflow.nodes.condition.summary', {
        count: Array.isArray(data.branches) ? data.branches.length : 0,
      });
    case 'human_gate':
      return t('workflow.nodes.humanGate.summary');
    case 'transform':
      return t('workflow.nodes.transform.summary', {
        mode: String(data.mode || 'template'),
      });
    case 'arena':
      return t('workflow.nodes.arena.summary', {
        count: Array.isArray(data.attempts) ? data.attempts.length : 0,
      });
  }
}

export function getWorkflowNodeVisual(
  kind: WorkflowNodeKind
): WorkflowNodeVisual {
  return NODE_VISUALS[kind];
}

function formatToken(value: unknown, fallback: string): string {
  return String(value || fallback).replace(/_/g, ' ');
}

export function getWorkflowNodeMetadata(
  kind: WorkflowNodeKind,
  data: WorkflowNodeData,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): WorkflowNodeMetadataChip[] {
  switch (kind) {
    case 'start':
      return [
        {
          label: t('workflow.metadata.trigger'),
          value: 'manual',
        },
      ];
    case 'end':
      return [
        {
          label: t('workflow.metadata.state'),
          value: 'terminal',
        },
      ];
    case 'agent':
      return [
        {
          label: t('workflow.metadata.role'),
          value: formatToken(data.role_template_id, 'custom'),
        },
      ];
    case 'condition':
      return [
        {
          label: t('workflow.metadata.branchCount'),
          value: t('workflow.metadata.branchCountValue', {
            count: Array.isArray(data.branches) ? data.branches.length : 0,
          }),
        },
        {
          label: t('workflow.metadata.routingMode'),
          value: formatToken(data.routing_mode, 'single'),
        },
      ];
    case 'human_gate':
      return [
        {
          label: t('workflow.metadata.action'),
          value: formatToken(data.required_action, 'approve or reject'),
        },
      ];
    case 'transform':
      return [
        {
          label: t('workflow.metadata.mode'),
          value: formatToken(data.mode, 'template'),
        },
      ];
    case 'arena':
      return [
        {
          label: t('workflow.metadata.attemptCount'),
          value: t('workflow.metadata.attemptCountValue', {
            count: Array.isArray(data.attempts) ? data.attempts.length : 0,
          }),
        },
        {
          label: t('workflow.metadata.promote'),
          value: formatToken(data.promote_strategy, 'manual'),
        },
      ];
  }
}

export function getWorkflowNodeRouteHints(
  kind: WorkflowNodeKind,
  data: WorkflowNodeData,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): WorkflowNodeRouteHint[] {
  if (kind === 'condition') {
    return (data.branches ?? []).slice(0, 3).map((branch, index) => ({
      label:
        branch.condition?.trim() ||
        branch.target_node_id ||
        t('workflow.metadata.branch', {
          index: index + 1,
        }),
      tone: index === 0 ? 'success' : 'warning',
    }));
  }

  if (kind === 'human_gate') {
    return [
      {
        label: t('workflow.edges.approve'),
        tone: 'success',
      },
      {
        label: t('workflow.edges.reject'),
        tone: 'danger',
      },
    ];
  }

  if (kind === 'arena') {
    return [{ label: t('workflow.edges.winner'), tone: 'brand' }];
  }

  return [];
}

export function getWorkflowEdgeLabel(
  kind: WorkflowEdgeKind,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): string | undefined {
  if (kind === 'default') {
    return undefined;
  }
  return t(EDGE_LABEL_I18N_KEYS[kind]);
}

export function getWorkflowEdgeVisual(
  kind: WorkflowEdgeKind,
  t: TFunction<'common'> = i18n.getFixedT('en', 'common')
): WorkflowEdgeVisual {
  const visual = EDGE_VISUALS[kind];
  if (kind === 'default') {
    return visual;
  }
  return {
    ...visual,
    label: getWorkflowEdgeLabel(kind, t),
  };
}

export function getWorkflowEdgeKindOptions(
  t: TFunction<'common'>
): WorkflowEdgeKindOption[] {
  return EDGE_KINDS.map((value) => ({
    value,
    label:
      value === 'default'
        ? t('workflow.edges.default')
        : (getWorkflowEdgeLabel(value, t) ?? t('workflow.edges.default')),
    description: t(`workflow.edges.options.${value}`),
  }));
}
