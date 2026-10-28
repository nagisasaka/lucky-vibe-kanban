import { Component, createContext, createRef, type ReactNode } from 'react';
import {
  SnapshotMotionGate,
  type SnapshotActivity,
} from '@/shared/lib/electric/snapshotActivity';
import type { KanbanItems } from './kanbanProjection';

export const ServerMotionContext = createContext({ stop: () => {} });
const TIMING = {
  duration: 400,
  easing: 'cubic-bezier(0.2, 0, 0, 1)',
  fill: 'both',
} as const;
type Card = {
  rect: DOMRect;
  column: string;
  clone?: HTMLElement;
  flying: boolean;
};
type Focus = { id: string; path: number[]; element: HTMLElement };
type Snapshot = { cards: Map<string, Card>; focus?: Focus };
type Props = {
  children: ReactNode;
  items: KanbanItems;
  scope: string;
  activity: SnapshotActivity;
  disabled: boolean;
};

/** Capture before React reparents cards. Only placement commits measure the board.
 * WAAPI owns the outer wrapper; dnd-kit exclusively owns the inner card transform.
 */
export class KanbanServerMotion extends Component<Props> {
  private root = createRef<HTMLDivElement>();
  private gate = new SnapshotMotionGate();
  private cleanups = new Set<() => void>();
  private unsubscribe?: () => void;
  private reduced?: MediaQueryList;
  private resize?: ResizeObserver;

  stop = () => {
    [...this.cleanups].forEach((cleanup) => cleanup());
  };
  private recover = () => {
    this.stop();
    this.gate.suspend(performance.now());
  };
  private observe = () => {
    if (!this.props.activity.state.healthy) this.recover();
    this.gate.observe(this.props.activity.state);
  };

  componentDidMount() {
    this.reduced = matchMedia('(prefers-reduced-motion: reduce)');
    // A cached successful generation may already exist while an initial fresh
    // request is in flight. That response must establish a quiet baseline too.
    if (!this.props.activity.pending)
      this.gate.observe(this.props.activity.state);
    this.unsubscribe = this.props.activity.subscribe(this.observe);
    document.addEventListener('visibilitychange', this.recover);
    window.addEventListener('offline', this.recover);
    window.addEventListener('online', this.recover);
    window.addEventListener('scroll', this.stop, true);
    window.addEventListener('resize', this.stop);
    this.reduced.addEventListener('change', this.stop);
    // Observe column widths, not card heights (summary/disclosure updates).
    const widths = new WeakMap<Element, number>();
    this.resize = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const previous = widths.get(entry.target);
        if (previous !== undefined && previous !== entry.contentRect.width)
          this.stop();
        widths.set(entry.target, entry.contentRect.width);
      }
    });
    this.root.current
      ?.querySelectorAll('[data-kanban-column]')
      .forEach((node) => this.resize?.observe(node));
  }
  componentWillUnmount() {
    this.stop();
    this.unsubscribe?.();
    this.resize?.disconnect();
    document.removeEventListener('visibilitychange', this.recover);
    window.removeEventListener('offline', this.recover);
    window.removeEventListener('online', this.recover);
    window.removeEventListener('scroll', this.stop, true);
    window.removeEventListener('resize', this.stop);
    this.reduced?.removeEventListener('change', this.stop);
  }

  getSnapshotBeforeUpdate(previous: Props): Snapshot | null {
    const changed =
      JSON.stringify(previous.items) !== JSON.stringify(this.props.items);
    if (!changed) {
      if (previous.scope !== this.props.scope || this.props.disabled)
        this.stop();
      return null;
    }
    const allowed =
      previous.scope === this.props.scope &&
      !previous.disabled &&
      !this.props.disabled &&
      !document.hidden &&
      !this.reduced?.matches &&
      this.gate.allows(this.props.activity.state) &&
      !document.querySelector('[data-kanban-ghost]');
    // Read the current animated rectangles before cancelling, so rapid updates
    // continue from the visible frame rather than replaying an old destination.
    const cards = new Map<string, Card>();
    const nextColumns = new Map(
      Object.entries(this.props.items).flatMap(([column, ids]) =>
        ids.map((id) => [id, column] as const)
      )
    );
    if (allowed)
      this.root.current
        ?.querySelectorAll<HTMLElement>('[data-kanban-motion]')
        .forEach((node) => {
          const rect = node.getBoundingClientRect();
          if (
            rect.bottom <= 0 ||
            rect.top >= innerHeight ||
            rect.right <= 0 ||
            rect.left >= innerWidth
          )
            return;
          const id = node.dataset.kanbanMotion!;
          const activeGhost = document.querySelector<HTMLElement>(
            `[data-kanban-server-ghost="${CSS.escape(id)}"]`
          );
          const column = node.closest<HTMLElement>('[data-kanban-column]')!
            .dataset.kanbanColumn!;
          cards.set(id, {
            rect: activeGhost?.getBoundingClientRect() ?? rect,
            column,
            flying: !!activeGhost,
            clone:
              activeGhost || nextColumns.get(id) !== column
                ? (node.cloneNode(true) as HTMLElement)
                : undefined,
          });
        });
    const active = document.activeElement;
    let focus: Focus | undefined;
    if (active instanceof HTMLElement && this.root.current?.contains(active)) {
      const card = active.closest<HTMLElement>('[data-kanban-motion]');
      if (card) {
        const path: number[] = [];
        for (
          let node: Element = active;
          node !== card && node.parentElement;
          node = node.parentElement
        )
          path.unshift([...node.parentElement.children].indexOf(node));
        focus = { id: card.dataset.kanbanMotion!, path, element: active };
      }
    }
    this.stop();
    return { cards, focus };
  }

  componentDidUpdate(
    _previous: Props,
    _state: unknown,
    snapshot: Snapshot | null
  ) {
    if (!snapshot) return;
    const root = this.root.current;
    if (!root) return;
    // The scroll viewport is the board boundary; ghosts cannot cover sticky headers.
    const viewport = root.parentElement!.getBoundingClientRect();
    const headers = [
      ...root.querySelectorAll<HTMLElement>('[data-kanban-header]'),
    ];
    const top = Math.max(
      0,
      viewport.top,
      ...headers.map((node) => node.getBoundingClientRect().bottom)
    );
    const clip = {
      left: Math.max(0, viewport.left),
      right: Math.min(innerWidth, viewport.right),
      top,
      bottom: Math.min(innerHeight, viewport.bottom),
    };
    const visible = (rect: DOMRect) =>
      rect.right > clip.left &&
      rect.left < clip.right &&
      rect.bottom > clip.top &&
      rect.top < clip.bottom;
    const destinations = [
      ...root.querySelectorAll<HTMLElement>('[data-kanban-motion]'),
    ].map((node) => ({ node, rect: node.getBoundingClientRect() }));
    destinations.forEach(({ node, rect }) => {
      const id = node.dataset.kanbanMotion!;
      const old = snapshot.cards.get(id);
      if (!old) return; // Filter entry/new cards have no invented origin.
      if (!visible(old.rect) || !visible(rect)) return;
      const dx = old.rect.left - rect.left,
        dy = old.rect.top - rect.top;
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) return;
      const crossColumn =
        old.flying ||
        old.column !==
          node.closest<HTMLElement>('[data-kanban-column]')!.dataset
            .kanbanColumn;
      let target = node;
      let layer: HTMLElement | undefined;
      if (crossColumn && old.clone) {
        layer = document.createElement('div');
        layer.setAttribute('aria-hidden', 'true');
        layer.setAttribute('inert', '');
        Object.assign(layer.style, {
          position: 'fixed',
          inset: '0',
          pointerEvents: 'none',
          zIndex: '19',
          clipPath: `inset(${clip.top}px ${innerWidth - clip.right}px ${innerHeight - clip.bottom}px ${clip.left}px)`,
        });
        target = old.clone;
        target.removeAttribute('data-kanban-motion');
        target.querySelectorAll('[data-kanban-card], [id]').forEach((child) => {
          child.removeAttribute('data-kanban-card');
          child.removeAttribute('id');
        });
        target.dataset.kanbanServerGhost = id;
        Object.assign(target.style, {
          position: 'absolute',
          left: `${rect.left}px`,
          top: `${rect.top}px`,
          width: `${rect.width}px`,
          height: `${rect.height}px`,
          transform: 'none',
          opacity: '1',
          pointerEvents: 'none',
        });
        layer.append(target);
        document.body.append(layer);
        node.style.opacity = '0';
        node.style.pointerEvents = 'none';
      }
      const animation = target.animate(
        [
          {
            transform: `translate(${dx}px, ${dy}px)`,
            ...(crossColumn
              ? { width: `${old.rect.width}px`, height: `${old.rect.height}px` }
              : {}),
          },
          {
            transform: 'translate(0, 0)',
            ...(crossColumn
              ? { width: `${rect.width}px`, height: `${rect.height}px` }
              : {}),
          },
        ],
        TIMING
      );
      node.dataset.kanbanAnimating = 'true';
      const cleanup = () => {
        animation.onfinish = null;
        animation.cancel();
        layer?.remove();
        node.style.removeProperty('opacity');
        node.style.removeProperty('pointer-events');
        delete node.dataset.kanbanAnimating;
        this.cleanups.delete(cleanup);
      };
      this.cleanups.add(cleanup);
      animation.onfinish = cleanup;
    });
    const focus = snapshot.focus;
    if (
      focus &&
      !focus.element.isConnected &&
      (document.activeElement === document.body ||
        document.activeElement === null)
    ) {
      let target: Element | undefined =
        root.querySelector(`[data-kanban-motion="${CSS.escape(focus.id)}"]`) ??
        undefined;
      for (const index of focus.path) target = target?.children[index];
      if (target instanceof HTMLElement) target.focus({ preventScroll: true });
    }
  }

  render() {
    return (
      <ServerMotionContext.Provider value={this}>
        <div
          ref={this.root}
          className="min-h-full"
          onPointerDownCapture={this.stop}
          onKeyDownCapture={this.stop}
        >
          {this.props.children}
        </div>
      </ServerMotionContext.Provider>
    );
  }
}
