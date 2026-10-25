import { describe, expect, it } from 'vitest';
import { aggregateToolStatus } from '@vibe/ui/lib/tool-lifecycle';

describe('aggregate tool lifecycle', () => {
  const entry = (status: string, active = false) => ({
    status: { status },
    active,
  });

  it.each(['failed', 'denied', 'timed_out', 'cancelled'])(
    'keeps %s visible among successful and running work',
    (status) => {
      expect(
        aggregateToolStatus([
          entry('success'),
          entry('created', true),
          entry(status),
        ])
      ).toEqual({ status });
    }
  );

  it('requires success evidence from every member', () => {
    expect(aggregateToolStatus([entry('success'), {}])).toEqual({
      status: 'unknown',
    });
    expect(aggregateToolStatus([entry('created')])).toEqual({
      status: 'unknown',
    });
    expect(aggregateToolStatus([])).toEqual({ status: 'unknown' });
    expect(aggregateToolStatus([entry('success'), entry('success')])).toEqual({
      status: 'success',
    });
    expect(
      aggregateToolStatus([entry('created', true), entry('success')])
    ).toEqual({
      status: 'created',
    });
  });
});
