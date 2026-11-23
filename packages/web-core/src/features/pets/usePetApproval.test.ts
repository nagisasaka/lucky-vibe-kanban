import { describe, expect, it } from 'vitest';
import { hasScopedPetApproval } from './usePetApproval';

describe('Pet approval scope', () => {
  const projects = [
    { project_id: 'a', local_workspace_id: 'one' },
    { project_id: 'a', local_workspace_id: 'two' },
    { project_id: 'b', local_workspace_id: 'other' },
  ];
  it('keeps waving until the last approval in the displayed project resolves', () => {
    for (const [pending, expected] of [
      [[], false],
      [['one', 'two'], true],
      [['two'], true],
      [[], false],
      [['other'], false],
    ] as const) {
      const workspaces = pending.map((id) => ({
        id,
        hasPendingRunApproval: true,
      }));
      expect(
        hasScopedPetApproval({ projectId: 'a' }, workspaces, projects)
      ).toBe(expected);
    }
  });
  it('uses exactly the selected workspace, including when changing screens', () => {
    const workspaces = [{ id: 'one', hasPendingRunApproval: true }];
    expect(
      hasScopedPetApproval({ workspaceId: 'one' }, workspaces, projects)
    ).toBe(true);
    expect(
      hasScopedPetApproval({ workspaceId: 'two' }, workspaces, projects)
    ).toBe(false);
    expect(
      hasScopedPetApproval({ workspaceId: undefined }, workspaces, projects)
    ).toBe(false);
  });
  it('does not treat input, review or Integration recovery as Run approval', () => {
    expect(
      hasScopedPetApproval(
        { workspaceId: 'one' },
        [{ id: 'one', hasPendingRunApproval: false }],
        projects
      )
    ).toBe(false);
    expect(
      hasScopedPetApproval({ workspaceId: 'one' }, [{ id: 'one' }], projects)
    ).toBe(false);
  });
});
