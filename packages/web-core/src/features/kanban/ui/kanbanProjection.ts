export type KanbanItems = Record<string, string[]>;
export type KanbanDestination = { droppableId: string; index: number };
type Rect = { left: number; top: number; width: number; height: number };

/** Project one card without mutating the canonical layout or persisting a move. */
export function projectKanbanItems(
  items: KanbanItems,
  moved: string,
  destination: KanbanDestination
): KanbanItems {
  if (
    !items[destination.droppableId] ||
    !Object.values(items).some((ids) => ids.includes(moved))
  )
    return items;
  const projected = Object.fromEntries(
    Object.entries(items).map(([id, ids]) => [
      id,
      ids.filter((item) => item !== moved),
    ])
  );
  const target = projected[destination.droppableId];
  target.splice(
    Math.max(0, Math.min(destination.index, target.length)),
    0,
    moved
  );
  return projected;
}

/** The same row-major slot is used for the hover gap and the committed rank. */
export function kanbanInsertionIndex({
  ids,
  overId,
  point,
  rects,
  columns,
}: {
  ids: string[];
  overId: string;
  point: { x: number; y: number } | null;
  rects: Map<string, Rect>;
  columns: number;
}): number {
  if (!ids.length) return 0;
  let index = ids.indexOf(overId);
  if (!point) return index < 0 ? ids.length : index;
  if (index < 0) {
    const measured = ids
      .map((id, i) => ({ i, rect: rects.get(id) }))
      .filter((entry): entry is { i: number; rect: Rect } => !!entry.rect);
    if (!measured.length) return ids.length;
    if (
      point.y >= Math.max(...measured.map(({ rect }) => rect.top + rect.height))
    )
      return ids.length;
    index = measured.reduce((nearest, entry) => {
      const distance = ({ rect }: typeof entry) =>
        Math.hypot(
          point.x - rect.left - rect.width / 2,
          point.y - rect.top - rect.height / 2
        );
      return distance(entry) < distance(nearest) ? entry : nearest;
    }).i;
  }
  const rect = rects.get(ids[index]);
  if (!rect) return index;
  const after =
    columns > 1
      ? point.x > rect.left + rect.width / 2
      : point.y > rect.top + rect.height / 2;
  return index + (after ? 1 : 0);
}
