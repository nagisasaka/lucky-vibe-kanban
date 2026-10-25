import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ChatElapsedTime } from '@vibe/ui/components/ChatElapsedTime';

describe('tool elapsed time evidence', () => {
  const render = (props: Parameters<typeof ChatElapsedTime>[0]) =>
    renderToStaticMarkup(createElement(ChatElapsedTime, props));

  it('does not fill missing or reversed endpoints with zero', () => {
    expect(render({ endedAt: '2026-10-25T00:00:04Z' })).toBe('');
    expect(render({ startedAt: '2026-10-25T00:00:01Z', active: false })).toBe(
      ''
    );
    expect(
      render({
        startedAt: '2026-10-25T00:00:04Z',
        endedAt: '2026-10-25T00:00:01Z',
      })
    ).toBe('');
    expect(render({ durationMs: -1 })).toBe('');
    expect(render({ durationMs: NaN })).toBe('');
  });

  it('uses provider duration even when start was not observed', () => {
    expect(render({ durationMs: 2500 })).toContain('2s');
    expect(render({ durationMs: 0 })).toContain('0s');
    expect(
      render({
        startedAt: '2026-10-25T00:00:01Z',
        endedAt: '2026-10-25T00:00:09Z',
        durationMs: 2500,
      })
    ).toContain('2s');
  });
});
