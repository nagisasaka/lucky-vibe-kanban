import { describe, expect, it } from 'vitest';
import { petStatus } from './petStatus';
describe('existing HTTP observations drive pet state', () => {
  it('does not interpret hidden, stale, absent or invalid counts as failure/running', () => {
    expect(petStatus(false, true, true, 2)).toBe('idle');
    expect(petStatus(true, false, false, 2)).toBe('idle');
    for (const count of [undefined, null, NaN, Infinity, -1, 0, 0.5])
      expect(petStatus(true, false, true, count)).toBe('idle');
  });
  it('prioritizes failure and reevaluates fresh sessions on recovery', () => {
    expect(petStatus(true, true, true, 2)).toBe('failed');
    expect(petStatus(true, false, true, 2)).toBe('running');
    expect(petStatus(true, false, false, 2)).toBe('idle');
  });
});
