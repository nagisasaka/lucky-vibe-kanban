/** Browser list search follows the existing Kanban trim/lowercase/includes rule. */
export function matchesListSearch(
  search: string,
  ...fields: string[]
): boolean {
  const query = search.trim().toLowerCase();
  return !query || fields.some((field) => field.toLowerCase().includes(query));
}
