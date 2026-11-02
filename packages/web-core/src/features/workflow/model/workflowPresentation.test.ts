import {
  formatScheduledTaskDateTime,
  scheduledTaskNoticeLabel,
} from './scheduledTaskPresentation';
import i18n from '@/i18n/config';
import { describe, expect, it } from 'vitest';
import {
  getWorkflowEdgeVisual,
  getWorkflowNodeMetadata,
  getWorkflowEdgeKindOptions,
  getWorkflowEdgeLabel,
  getWorkflowNodeKindLabel,
  getWorkflowNodeRouteHints,
  getWorkflowNodeSummary,
  getWorkflowNodeVisual,
} from './workflowPresentation';

describe('workflow presentation helpers', () => {
  it('returns human-readable node kind labels', () => {
    expect(getWorkflowNodeKindLabel('human_gate')).toBe('Human Gate');
    expect(getWorkflowNodeKindLabel('arena')).toBe('Arena');
  });

  it('returns compact node summaries for workflow nodes', () => {
    expect(
      getWorkflowNodeSummary('agent', { role_template_id: 'reviewer' })
    ).toBe('Role: reviewer');
    expect(
      getWorkflowNodeSummary('condition', {
        branches: [
          { condition: 'Matched', target_node_id: 'raw-matched' },
          { condition: 'Fallback', target_node_id: 'raw-fallback' },
        ],
      })
    ).toBe('2 branches');
    expect(getWorkflowNodeSummary('human_gate', {})).toBe('Waits for approval');
  });

  it('returns semantic labels for non-default edges only', () => {
    expect(getWorkflowEdgeLabel('default')).toBeUndefined();
    expect(getWorkflowEdgeLabel('condition_branch')).toBe('Condition');
    expect(getWorkflowEdgeLabel('approval')).toBe('Approve');
    expect(getWorkflowEdgeLabel('rejection')).toBe('Reject');
    expect(getWorkflowEdgeLabel('arena_winner')).toBe('Winner');
  });

  it('returns ordered edge kind options for the edge inspector', () => {
    expect(
      getWorkflowEdgeKindOptions(i18n.getFixedT('en', 'common')).map(
        (option) => option.value
      )
    ).toEqual([
      'default',
      'condition_branch',
      'approval',
      'rejection',
      'arena_winner',
    ]);
  });

  it('returns stable visual tokens for workflow node kinds', () => {
    expect(getWorkflowNodeVisual('condition')).toMatchObject({
      accentClass: expect.stringContaining('amber'),
      iconClass: expect.stringContaining('amber'),
    });
    expect(getWorkflowNodeVisual('human_gate')).toMatchObject({
      accentClass: expect.stringContaining('violet'),
      iconClass: expect.stringContaining('violet'),
    });
  });

  it('returns compact metadata chips for workflow nodes', () => {
    expect(
      getWorkflowNodeMetadata('agent', {
        role_template_id: 'reviewer',
        output_capture: 'diff_summary',
      })
    ).toEqual([{ label: 'Role', value: 'reviewer' }]);

    expect(
      getWorkflowNodeMetadata('condition', {
        branches: [
          { target_node_id: 'matched', condition: 'Matched work' },
          { target_node_id: 'fallback', condition: 'Fallback work' },
        ],
        routing_mode: 'multi',
      })
    ).toEqual([
      { label: 'Branches', value: '2 branches' },
      { label: 'Routing', value: 'multi' },
    ]);

    expect(
      getWorkflowNodeMetadata('arena', {
        attempts: [{ id: 'a' }, { id: 'b' }],
        promote_strategy: 'manual',
      })
    ).toEqual([
      { label: 'Attempts', value: '2 attempts' },
      { label: 'Promote', value: 'manual' },
    ]);
  });

  it('returns route hints for branching node kinds', () => {
    expect(
      getWorkflowNodeRouteHints('condition', {
        branches: [
          { target_node_id: 'true', condition: 'Ready to ship' },
          { target_node_id: 'fallback' },
        ],
      })
    ).toEqual([
      { label: 'Ready to ship', tone: 'success' },
      { label: 'fallback', tone: 'warning' },
    ]);
    expect(getWorkflowNodeRouteHints('human_gate', {})).toEqual([
      { label: 'Approve', tone: 'success' },
      { label: 'Reject', tone: 'danger' },
    ]);
    expect(getWorkflowNodeRouteHints('arena', {})).toEqual([
      { label: 'Winner', tone: 'brand' },
    ]);
  });

  it('returns visual tokens for semantic edge kinds', () => {
    expect(getWorkflowEdgeVisual('approval')).toMatchObject({
      label: 'Approve',
      pathClass: expect.stringContaining('emerald'),
      chipClass: expect.stringContaining('emerald'),
    });
    expect(getWorkflowEdgeVisual('rejection')).toMatchObject({
      label: 'Reject',
      pathClass: expect.stringContaining('rose'),
      chipClass: expect.stringContaining('rose'),
    });
  });
});

it.each([
  ['en', 'UTC', 'Jan 2, 2026, 3:04 PM'],
  ['ja', 'UTC', '2026/01/02 15:04'],
  ['ja', 'Asia/Shanghai', '2026/01/02 23:04'],
  ['en', 'America/New_York', 'Jan 2, 2026, 10:04 AM'],
])(
  'formats scheduled times in %s while preserving %s',
  (locale, timezone, expected) => {
    expect(
      formatScheduledTaskDateTime('2026-01-02T15:04:05Z', timezone, '—', locale)
    ).toBe(expected);
    expect(formatScheduledTaskDateTime('invalid', timezone, '—', locale)).toBe(
      '—'
    );
    expect(formatScheduledTaskDateTime(null, timezone, '—', locale)).toBe('—');
  }
);

it('resolves an existing scheduled-task notice in the current display language', () => {
  const code = 'runFailed';
  const english = scheduledTaskNoticeLabel(
    code,
    i18n.getFixedT('en', 'common')
  );
  const japanese = scheduledTaskNoticeLabel(
    code,
    i18n.getFixedT('ja', 'common')
  );
  expect(english).toContain('could not be confirmed');
  expect(japanese).toContain('開始結果を確認できません');
  expect(japanese).not.toBe(english);
});
