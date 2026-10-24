/** Match the sidebar's normalized query against already available metadata. */
export function matchesWorkspaceSearch(
  workspace: { name: string; branch: string },
  normalizedQuery: string,
  issueSimpleId?: string | null
): boolean {
  return (
    workspace.name.toLowerCase().includes(normalizedQuery) ||
    workspace.branch.toLowerCase().includes(normalizedQuery) ||
    (issueSimpleId?.toLowerCase().includes(normalizedQuery) ?? false)
  );
}
