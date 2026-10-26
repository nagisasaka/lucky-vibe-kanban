/** Rank only the dragged Issue; never renumber siblings or overwrite their activity. */
export function dropRank({
  before,
  after,
  descending,
  promoted,
  destinationRanks,
}: {
  before?: number;
  after?: number;
  descending: boolean;
  promoted: boolean;
  destinationRanks: number[];
}): number {
  if (promoted)
    return descending
      ? Math.max(0, ...destinationRanks) + 1
      : Math.min(0, ...destinationRanks) - 1;
  const direction = descending ? -1 : 1;
  if (before === undefined) return (after ?? direction) - direction;
  if (after === undefined) return before + direction;
  return (before + after) / 2;
}

/** Resolve insertion against the frozen order, excluding activity-promoted peers. */
export function dropRankAfterActivity({
  ids,
  index,
  moved,
  startOrders,
  currentOrders,
  descending,
  manual,
}: {
  ids: string[];
  index: number;
  moved: string;
  startOrders: Record<string, number>;
  currentOrders: Record<string, number>;
  descending: boolean;
  manual: boolean;
}): number {
  const promoted = (id: string) =>
    manual &&
    startOrders[id] !== undefined &&
    (descending
      ? currentOrders[id] > startOrders[id]
      : currentOrders[id] < startOrders[id]);
  const stable = (id: string) =>
    id !== moved && currentOrders[id] !== undefined && !promoted(id);
  const beforeId = ids.slice(0, index).reverse().find(stable);
  const afterId = ids.slice(index + 1).find(stable);
  const promotedRanks = Object.keys(currentOrders)
    .filter(
      (id) =>
        id !== moved &&
        (!afterId ||
          (descending
            ? currentOrders[id] > currentOrders[afterId]
            : currentOrders[id] < currentOrders[afterId]))
    )
    .map((id) => currentOrders[id]);
  // Event promotions form a prefix. Inserting ahead of the remaining stable
  // cards must follow that prefix, not tie with or overtake it.
  const before = beforeId
    ? currentOrders[beforeId]
    : promotedRanks.length
      ? descending
        ? Math.min(...promotedRanks)
        : Math.max(...promotedRanks)
      : undefined;
  return dropRank({
    before,
    after: afterId ? currentOrders[afterId] : undefined,
    descending,
    promoted: promoted(moved),
    destinationRanks: Object.values(currentOrders),
  });
}
