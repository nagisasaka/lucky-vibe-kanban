import { describe, expect, it } from 'vitest';
import { acknowledgeVersionSave } from './issue-version-drafts';

const captured = {
  name: 'draft1',
  description: 'first',
  revision: 1,
  saved: false,
};
describe('version save acknowledgements', () => {
  it('marks persisted content saved only after the acknowledgement', () => {
    expect(captured.saved).toBe(false);
    expect(acknowledgeVersionSave(captured, captured, 2)).toEqual({
      ...captured,
      revision: 2,
      saved: true,
    });
  });
  it('preserves text typed during a pending save and advances its base revision', () => {
    const later = { ...captured, description: 'second' };
    expect(acknowledgeVersionSave(later, captured, 2)).toEqual({
      ...later,
      revision: 2,
      saved: false,
    });
  });
  it('preserves a concurrent local rename as unsaved', () => {
    const later = { ...captured, name: 'fix' };
    expect(acknowledgeVersionSave(later, captured, 2)).toEqual({
      ...later,
      revision: 2,
      saved: false,
    });
  });
});
