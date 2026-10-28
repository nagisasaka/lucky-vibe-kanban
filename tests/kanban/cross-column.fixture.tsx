import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import {
  KanbanGrid,
  KanbanGridCards,
  KanbanGridCard,
} from "../../packages/web-core/src/features/kanban/ui/KanbanGrid";
import {
  projectKanbanItems,
  type KanbanItems,
} from "../../packages/web-core/src/features/kanban/ui/kanbanProjection";

function Fixture() {
  const [items, setItems] = useState<KanbanItems>({
    A: ["a", "b"],
    B: ["c", "d", "e"],
    C: [],
  });
  const [preview, setPreview] = useState<KanbanItems | null>(null);
  return (
    <KanbanGrid
      onDragPreview={
        window.fixedSort
          ? undefined
          : (result) =>
              setPreview(
                result?.destination
                  ? projectKanbanItems(
                      items,
                      result.draggableId,
                      result.destination,
                    )
                  : null,
              )
      }
      onDragEnd={(result) => {
        window.drops.push(result);
        if (result.destination)
          setItems(
            projectKanbanItems(items, result.draggableId, result.destination),
          );
      }}
    >
      {Object.entries(preview ?? items).map(([column, ids]) => (
        <section key={column}>
          <KanbanGridCards id={column} items={ids}>
            {ids.map((id) => (
              <KanbanGridCard
                key={id}
                id={id}
                name={id}
                onClick={() => {
                  window.clicks++;
                }}
              >
                <div style={{ height: id === "d" ? 110 : 70 }}>{id}</div>
              </KanbanGridCard>
            ))}
          </KanbanGridCards>
        </section>
      ))}
    </KanbanGrid>
  );
}
window.drops = [];
window.clicks = 0;
createRoot(document.getElementById("root")!).render(<Fixture />);
