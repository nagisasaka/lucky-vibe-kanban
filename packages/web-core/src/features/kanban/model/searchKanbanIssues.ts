import type { Issue, IssueRelationship } from 'shared/remote-types';

// Only the authorised issue collection may supply graph vertices. Never fetch
// missing endpoints: they may be outside the project or the actor's access.
export function searchKanbanIssues(
  issues: Issue[],
  relationships: IssueRelationship[],
  searchQuery: string,
  includeRelatedIssues: boolean
): Set<string> {
  const query = searchQuery.trim().toLowerCase();
  if (!query) return new Set(issues.map((issue) => issue.id));

  const visited = new Set(
    issues
      .filter(
        (issue) =>
          issue.title.toLowerCase().includes(query) ||
          issue.simple_id.toLowerCase().includes(query) ||
          String(issue.issue_number).includes(query)
      )
      .map((issue) => issue.id)
  );
  if (!includeRelatedIssues || visited.size === 0) return visited;

  const byId = new Map(issues.map((issue) => [issue.id, issue]));
  const neighbors = new Map<string, string[]>();
  for (const relationship of relationships) {
    if (relationship.relationship_type !== 'related') continue;
    const source = byId.get(relationship.issue_id);
    const target = byId.get(relationship.related_issue_id);
    if (!source || !target || source.project_id !== target.project_id) continue;
    if (!neighbors.has(source.id)) neighbors.set(source.id, []);
    if (!neighbors.has(target.id)) neighbors.set(target.id, []);
    neighbors.get(source.id)!.push(target.id);
    neighbors.get(target.id)!.push(source.id);
  }

  const queue = [...visited];
  for (let index = 0; index < queue.length; index++) {
    for (const id of neighbors.get(queue[index]) ?? []) {
      if (visited.has(id)) continue;
      visited.add(id);
      queue.push(id);
    }
  }
  return visited;
}
