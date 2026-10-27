import { createPortal } from 'react-dom';
import { useState, useRef, type ReactNode, type MouseEvent } from 'react';
import {
  DndContext,
  DragOverlay,
  defaultDropAnimationSideEffects,
  MouseSensor,
  TouchSensor,
  KeyboardSensor,
  useSensor,
  useSensors,
  useDroppable,
  pointerWithin,
  closestCenter,
  type CollisionDetection,
} from '@dnd-kit/core';
import {
  SortableContext,
  useSortable,
  sortableKeyboardCoordinates,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { DropResult } from '@vibe/ui/components/KanbanBoard';
import { cn } from '@/shared/lib/utils';
import './kanban-grid.css';

// Keep sortable displacement and overlay landing in sync. Override only in the
// Workspace's Vite environment to compare candidates before choosing the final feel.
const KANBAN_MOTION_PRESETS = {
  crisp: { duration: 200, easing: 'cubic-bezier(0.2, 0, 0, 1)' },
  bouncy: { duration: 200, easing: 'cubic-bezier(0.34, 1.56, 0.64, 1)' },
} as const;
const KANBAN_MOTION =
  KANBAN_MOTION_PRESETS[
    import.meta.env.VITE_KANBAN_MOTION === 'bouncy' ? 'bouncy' : 'crisp'
  ];
const DROP_ANIMATION = {
  ...KANBAN_MOTION,
  sideEffects: defaultDropAnimationSideEffects({
    styles: { active: { opacity: '0' } },
  }),
};

// Prefer the card under the pointer, then the column (including an empty one).
// Keyboard dragging uses the same measured card rectangles in row-major order.
const collisionDetection: CollisionDetection = (args) => {
  const hits = pointerWithin(args);
  if (args.pointerCoordinates) {
    const cards = hits.filter(
      (hit) =>
        args.droppableContainers.find((container) => container.id === hit.id)
          ?.data.current?.sortable
    );
    return cards.length ? cards : hits;
  }
  return closestCenter({
    ...args,
    droppableContainers: args.droppableContainers.filter(
      (container) =>
        container.data.current?.sortable || container.data.current?.count === 0
    ),
  });
};

export function KanbanGrid({
  children,
  onDragEnd,
  onDraggingChange,
}: {
  children: ReactNode;
  onDragEnd: (result: DropResult) => void;
  onDraggingChange?: (dragging: boolean) => void;
}) {
  const [overlay, setOverlay] = useState<ReactNode>(null);
  const finish = () => {
    setOverlay(null);
    onDraggingChange?.(false);
  };
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 6 } }),
    useSensor(TouchSensor, {
      activationConstraint: { delay: 200, tolerance: 5 },
    }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  return (
    <DndContext
      sensors={sensors}
      collisionDetection={collisionDetection}
      onDragStart={({ active }) => {
        setOverlay(active.data.current?.overlay ?? null);
        onDraggingChange?.(true);
      }}
      onDragCancel={finish}
      onDragEnd={({ active, over, activatorEvent }) => {
        finish();
        const source = active.data.current?.sortable;
        if (!source || !over) return;
        const destination = over.data.current?.sortable;
        onDragEnd({
          draggableId: String(active.id),
          type: 'DEFAULT',
          reason: 'DROP',
          mode: 'FLUID',
          combine: null,
          source: { droppableId: source.containerId, index: source.index },
          destination: {
            droppableId: destination?.containerId ?? String(over.id),
            index: destination?.index ?? over.data.current?.count ?? 0,
          },
        });
        if (activatorEvent instanceof KeyboardEvent) {
          requestAnimationFrame(() =>
            document
              .querySelector<HTMLElement>(
                `[data-kanban-card="${window.CSS.escape(String(active.id))}"]`
              )
              ?.focus()
          );
        }
      }}
    >
      <div className="inline-grid grid-flow-col auto-cols-[minmax(260px,1fr)] divide-x border-x items-stretch min-h-full min-w-full">
        {children}
      </div>
      {createPortal(
        <DragOverlay dropAnimation={DROP_ANIMATION}>
          {overlay && (
            <div
              data-kanban-ghost
              aria-hidden="true"
              ref={(node) => node?.setAttribute('inert', '')}
              className="group min-w-0 border bg-primary p-base rounded-sm shadow-lg pointer-events-none"
            >
              {overlay}
            </div>
          )}
        </DragOverlay>,
        document.body
      )}
    </DndContext>
  );
}

export function KanbanGridCards({
  id,
  items,
  children,
}: {
  id: string;
  items: string[];
  children: ReactNode;
}) {
  const { setNodeRef, isOver } = useDroppable({
    id,
    data: { count: items.length },
  });
  return (
    <SortableContext id={id} items={items}>
      <div className="kanban-grid-column flex-1 min-w-0">
        <div
          ref={setNodeRef}
          data-kanban-column={id}
          className={cn(
            'kanban-grid-cards min-h-40 h-full content-start',
            isOver && 'bg-brand/5'
          )}
        >
          {children}
        </div>
      </div>
    </SortableContext>
  );
}

export function KanbanGridCard({
  id,
  name,
  children,
  onClick,
  isOpen,
  isSelected,
  dragDisabled,
}: {
  id: string;
  name: string;
  children: ReactNode;
  onClick: (event: MouseEvent<HTMLDivElement>) => void;
  isOpen?: boolean;
  isSelected?: boolean;
  dragDisabled?: boolean;
}) {
  const {
    setNodeRef,
    attributes,
    listeners,
    isDragging,
    transform,
    transition,
  } = useSortable({
    id,
    disabled: dragDisabled,
    transition: KANBAN_MOTION,
    data: { overlay: children },
  });
  const dragged = useRef(false);
  if (isDragging) dragged.current = true;
  return (
    <div
      ref={setNodeRef}
      data-kanban-card={id}
      style={{
        transform: CSS.Translate.toString(transform),
        transition,
        opacity: isDragging ? 0 : undefined,
      }}
      className={cn(
        'group min-w-0 self-start border bg-primary p-base rounded-sm focus-visible:ring-2 focus-visible:ring-brand outline-none',
        isDragging && 'relative z-30 shadow-lg',
        isSelected && 'ring-2 ring-accent ring-inset',
        isOpen && 'ring-2 ring-secondary-foreground ring-inset'
      )}
      {...attributes}
      {...listeners}
      aria-label={name}
      onMouseDown={(event) => {
        dragged.current = false;
        const control = (event.target as HTMLElement).closest(
          'button, a, input, textarea, summary, [role="button"]'
        );
        if (control && control !== event.currentTarget) return;
        listeners?.onMouseDown?.(event);
      }}
      onTouchStart={(event) => {
        dragged.current = false;
        const control = (event.target as HTMLElement).closest(
          'button, a, input, textarea, summary, [role="button"]'
        );
        if (control && control !== event.currentTarget) return;
        listeners?.onTouchStart?.(event);
      }}
      onClick={(event) => {
        if (!dragged.current) onClick(event);
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === 'Enter' && !isDragging) {
          event.preventDefault();
          onClick(event as unknown as MouseEvent<HTMLDivElement>);
        } else listeners?.onKeyDown?.(event);
      }}
    >
      {children}
    </div>
  );
}
