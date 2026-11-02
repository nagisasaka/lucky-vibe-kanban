import i18n from '@/i18n/config';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentGoalStatus, type AgentGoalState } from 'shared/types';
import { GoalProgressCard } from './GoalProgressCard';

function render(status: AgentGoalStatus, active = false, disabled = false) {
  return renderToStaticMarkup(
    createElement(GoalProgressCard, {
      goal: {
        objective: 'Continue investigation',
        status,
        tokens_used: 123,
        time_used_seconds: 45,
        token_budget: null,
      } as AgentGoalState,
      isRunActive: active,
      onPause: vi.fn(),
      onResume: vi.fn(),
      onEdit: vi.fn(),
      onClear: vi.fn(),
      onInterruptTurn: vi.fn(),
      onResumeSaved: vi.fn(),
      resumeSavedDisabled: disabled,
    })
  );
}

describe('Saved Goal actions', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('en');
  });
  it('offers resume even when the last recorded native status was active', () => {
    const html = render(AgentGoalStatus.active);
    expect(html).toContain('Resume Goal');
    expect(html).toContain('Recorded time');
    expect(html).not.toContain('Stop current turn');
  });
  it('never offers resume for a complete saved Goal', () => {
    expect(render(AgentGoalStatus.complete)).not.toContain('Resume Goal');
  });
  it('disables saved resume when the session is unavailable', () => {
    expect(render(AgentGoalStatus.paused, false, true)).toMatch(/disabled=""/);
  });
  it('keeps live controls separate from starting a new run', () => {
    const html = render(AgentGoalStatus.paused, true);
    expect(html).toContain('Stop current turn');
    expect(html).not.toContain('Resume Goal');
  });
});

it('renders saved Goal controls in Japanese without translating its objective', async () => {
  await i18n.changeLanguage('ja');
  const html = render(AgentGoalStatus.blocked);
  expect(html).toContain('保存済みのGoal');
  expect(html).toContain('Goalを再開');
  expect(html).toContain('ブロック中');
  expect(html).toContain('Continue investigation');
  expect(html).toContain('0分45秒');
});
