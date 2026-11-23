import type {
  MachineFilter,
  SavedFilterDefinition,
  SavedFilterEvaluation,
} from 'shared/remote-types';

export const emptyMachineFilter = (): MachineFilter => ({
  search: null,
  status_ids: [],
  priorities: [],
  assignee_ids: [],
  tag_ids: [],
  tag_mode: 'any',
  archived: 'active',
  created_at: null,
  updated_at: null,
  relation: null,
});
export const emptyDefinition = (): SavedFilterDefinition => ({
  name: '',
  machine: emptyMachineFilter(),
  natural_language: null,
  context: null,
});
export function savedFilterHref(projectId: string, id: string) {
  return `/projects/${encodeURIComponent(projectId)}?savedFilter=${encodeURIComponent(id)}`;
}

// A partial page is never a board result. A stale response is displayable only
// when the service supplies a previous result; initial failure is not zero matches.
export function hasEvaluationResult(result: SavedFilterEvaluation | undefined) {
  return (
    !!result &&
    (result.state === 'complete' ||
      (result.stale && result.issue_ids.length > 0))
  );
}
