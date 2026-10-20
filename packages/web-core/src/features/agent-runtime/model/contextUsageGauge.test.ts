import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  ContextUsageGauge,
  contextUsagePercentage,
} from '@vibe/ui/components/ContextUsageGauge';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, args?: { percentage?: number }) =>
      `${key}${args?.percentage == null ? '' : `: ${args.percentage}%`}`,
  }),
}));

describe('context occupancy', () => {
  it('uses context usage independently from cumulative tokens', () => {
    expect(
      contextUsagePercentage({
        total_tokens: 25,
        model_context_window: 100,
        cumulative_tokens: 12000,
      })
    ).toBe(25);
    expect(
      contextUsagePercentage({ total_tokens: 0, model_context_window: 100 })
    ).toBe(0);
    expect(
      contextUsagePercentage({ total_tokens: 120, model_context_window: 100 })
    ).toBe(100);
  });
  it('never invents zero percent from unavailable or invalid measurements', () => {
    expect(contextUsagePercentage(null)).toBeNull();
    for (const value of [-1, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
      expect(
        contextUsagePercentage({
          total_tokens: value,
          model_context_window: 100,
        })
      ).toBeNull();
      expect(
        contextUsagePercentage({
          total_tokens: 10,
          model_context_window: value,
        })
      ).toBeNull();
    }
    expect(
      contextUsagePercentage({ total_tokens: 10, model_context_window: 0 })
    ).toBeNull();
  });
  it('renders unknown without a zero-percent label and labels a real measurement', () => {
    const unknown = renderToStaticMarkup(
      createElement(ContextUsageGauge, { tokenUsageInfo: null })
    );
    expect(unknown).toContain('—');
    expect(unknown).not.toContain('0%');
    const measured = renderToStaticMarkup(
      createElement(ContextUsageGauge, {
        tokenUsageInfo: { total_tokens: 25, model_context_window: 100 },
      })
    );
    expect(measured).toContain('25%');
  });
});
