import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import {
  KanbanGrid,
  KanbanGridCards,
  KanbanGridCard,
} from "../../packages/web-core/src/features/kanban/ui/KanbanGrid";
import { KanbanServerMotion } from "../../packages/web-core/src/features/kanban/ui/KanbanServerMotion";
import { SnapshotActivity } from "../../packages/web-core/src/shared/lib/electric/snapshotActivity";
import type { KanbanItems } from "../../packages/web-core/src/features/kanban/ui/kanbanProjection";
const activity = new SnapshotActivity();
if (window.cached) {
  activity.report(performance.now(), true);
  activity.begin();
}
function Fixture() {
  const [items, setItems] = useState<KanbanItems>({
    A: ["a", "b", "c"],
    B: ["d", "e"],
    C: [],
  });
  const [scope, setScope] = useState("manual");
  const [dragging, setDragging] = useState(false);
  window.change = (
    next: KanbanItems,
    nextScope?: string,
    startedAt = performance.now(),
  ) => {
    activity.report(startedAt, true);
    setItems(next);
    if (nextScope) setScope(nextScope);
  };
  window.fail = () => activity.report(performance.now(), false);
  return (
    <div id="scroll" style={{ overflow: "auto", width: "100%", height: 650 }}>
      <KanbanServerMotion
        items={items}
        scope={scope}
        activity={activity}
        disabled={dragging}
      >
        <KanbanGrid
          onDraggingChange={setDragging}
          onDragEnd={(r) => window.drops.push(r)}
        >
          {Object.entries(items).map(([column, ids]) => (
            <section key={column} style={{ width: window.columnWidth ?? 360 }}>
              <header
                data-kanban-header
                style={{
                  position: "sticky",
                  top: 0,
                  zIndex: 20,
                  height: 30,
                  background: "white",
                }}
              >
                {column} ({ids.length})
              </header>
              <KanbanGridCards id={column} items={ids}>
                {ids.map((id) => (
                  <KanbanGridCard
                    key={id}
                    id={id}
                    name={id}
                    onClick={() => window.clicks++}
                  >
                    <div style={{ height: id === "b" ? 110 : 70 }}>
                      {id}
                      <button aria-label={"control-" + id}>control</button>
                    </div>
                  </KanbanGridCard>
                ))}
              </KanbanGridCards>
            </section>
          ))}
        </KanbanGrid>
      </KanbanServerMotion>
    </div>
  );
}
window.drops = [];
window.clicks = 0;
createRoot(document.getElementById("root")!).render(<Fixture />);
