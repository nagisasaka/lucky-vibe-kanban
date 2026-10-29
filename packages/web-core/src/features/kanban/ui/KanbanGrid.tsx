import { createPortal } from 'react-dom';
import { ServerMotionContext } from './KanbanServerMotion';
import {
  createContext,
  useContext,
  useState,
  useRef,
  type ReactNode,
  type MouseEvent,
} from 'react';
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
  type DragMoveEvent,
  type DragOverEvent,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  useSortable,
  sortableKeyboardCoordinates,
  defaultAnimateLayoutChanges,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { kanbanInsertionIndex } from './kanbanProjection';
import type { DropResult } from '@vibe/ui/components/KanbanBoard';
import { cn } from '@/shared/lib/utils';
import { useTouchClickGuard } from './useTouchClickGuard';
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
const CrossColumnMotionContext = createContext(false);
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
  onDragPreview,
}: {
  children: ReactNode;
  onDragEnd: (result: DropResult) => void;
  onDraggingChange?: (dragging: boolean) => void;
  onDragPreview?: (result: DropResult | null) => void;
}) {
  const serverMotion = useContext(ServerMotionContext);
  const [overlay, setOverlay] = useState<ReactNode>(null);
  const origin = useRef<{ droppableId: string; index: number } | null>(null);
  const preview = useRef<DropResult | null>(null);
  const pointer = useRef<{ x: number; y: number } | null>(null);
  const layout = useRef(
    new Map<
      string,
      {
        ids: string[];
        rects: Map<string, DOMRect>;
        columns: number;
        scroll: { x: number; y: number };
      }
    >()
  );
  const scrollPosition = (element: HTMLElement) => {
    let x = 0,
      y = 0;
    for (
      let node: HTMLElement | null = element;
      node;
      node = node.parentElement
    ) {
      x += node.scrollLeft;
      y += node.scrollTop;
    }
    return { x, y };
  };
  const updatePreview = (
    event: DragMoveEvent | DragOverEvent | DragEndEvent
  ) => {
    const { active, over } = event;
    if (!onDragPreview || !origin.current || !over) return null;
    // Sensor pointer coordinates exclude scroll/layout corrections in delta.
    const point = pointer.current;
    // Keyboard coordinates can land on the projected active card itself.
    if (!point && over.id === active.id) return preview.current;
    const sortable = over.data.current?.sortable;
    const pointedColumn = point
      ? document
          .elementFromPoint(point.x, point.y)
          ?.closest<HTMLElement>('[data-kanban-column]')?.dataset.kanbanColumn
      : undefined;
    const column = pointedColumn ?? String(sortable?.containerId ?? over.id);
    if (column === origin.current.droppableId && !preview.current) return null;
    const element = document.querySelector<HTMLElement>(
      `[data-kanban-column="${window.CSS.escape(column)}"]`
    );
    if (!element) return null;
    const frozen = layout.current.get(column);
    if (!frozen) return null;
    const scroll = scrollPosition(element);
    const rects = new Map(
      [...frozen.rects].map(([id, rect]) => [
        id,
        {
          left: rect.left - scroll.x + frozen.scroll.x,
          top: rect.top - scroll.y + frozen.scroll.y,
          width: rect.width,
          height: rect.height,
        },
      ])
    );
    const index = kanbanInsertionIndex({
      ids: frozen.ids.filter((id) => id !== String(active.id)),
      // Use frozen geometry for pointer slots: projected reflow must not move
      // the hit targets and oscillate the gap under a stationary pointer.
      overId: point ? column : String(over.id),
      point,
      rects,
      columns: frozen.columns,
    });
    const result: DropResult = {
      draggableId: String(active.id),
      type: 'DEFAULT',
      reason: 'DROP',
      mode: 'FLUID',
      combine: null,
      source: origin.current,
      destination: { droppableId: column, index },
    };
    if (
      preview.current?.destination?.droppableId !== column ||
      preview.current.destination.index !== index
    ) {
      preview.current = result;
      onDragPreview(result);
    }
    return result;
  };
  const finish = () => {
    origin.current = null;
    preview.current = null;
    layout.current.clear();
    onDragPreview?.(null);
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
      collisionDetection={(args) => {
        pointer.current = args.pointerCoordinates;
        return collisionDetection(args);
      }}
      onDragMove={updatePreview}
      onDragOver={updatePreview}
      onDragStart={({ active }) => {
        serverMotion.stop();
        layout.current.clear();
        document
          .querySelectorAll<HTMLElement>('[data-kanban-column]')
          .forEach((element) => {
            const nodes = [
              ...element.querySelectorAll<HTMLElement>('[data-kanban-card]'),
            ];
            layout.current.set(element.dataset.kanbanColumn!, {
              ids: nodes.map((node) => node.dataset.kanbanCard!),
              rects: new Map(
                nodes.map((node) => [
                  node.dataset.kanbanCard!,
                  node.getBoundingClientRect(),
                ])
              ),
              columns:
                getComputedStyle(element).gridTemplateColumns.split(' ').length,
              scroll: scrollPosition(element),
            });
          });
        const sortable = active.data.current?.sortable;
        origin.current = sortable
          ? { droppableId: String(sortable.containerId), index: sortable.index }
          : null;
        setOverlay(active.data.current?.overlay ?? null);
        onDraggingChange?.(true);
      }}
      onDragCancel={({ active, activatorEvent }) => {
        finish();
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
      onDragEnd={(event) => {
        const { active, over, activatorEvent } = event;
        const projected = updatePreview(event);
        const source = origin.current;
        finish();
        if (!source || !over) return;
        const destination = over.data.current?.sortable;
        onDragEnd(
          projected ?? {
            draggableId: String(active.id),
            type: 'DEFAULT',
            reason: 'DROP',
            mode: 'FLUID',
            combine: null,
            source,
            destination: {
              droppableId: destination?.containerId ?? String(over.id),
              index: destination?.index ?? over.data.current?.count ?? 0,
            },
          }
        );
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
      <CrossColumnMotionContext.Provider value={preview.current !== null}>
        <div className="inline-grid grid-flow-col auto-cols-[minmax(260px,1fr)] divide-x border-x items-stretch min-h-full min-w-full">
          {children}
        </div>
      </CrossColumnMotionContext.Provider>
      {createPortal(
        <DragOverlay
          dropAnimation={DROP_ANIMATION}
          style={{ pointerEvents: 'none' }}
        >
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
  const crossColumnMotion = useContext(CrossColumnMotionContext);
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
    animateLayoutChanges: (args) =>
      (crossColumnMotion && args.isSorting) ||
      defaultAnimateLayoutChanges(args),
    data: { overlay: children },
  });
  const touchClickGuard = useTouchClickGuard();
  const dragged = useRef(false);
  if (isDragging) dragged.current = true;
  return (
    <div data-kanban-motion={id} className="min-w-0 self-start">
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
        {...touchClickGuard}
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
    </div>
  );
}
