import { useLayoutEffect, useRef, useState } from 'react';
import { KanbanBadge } from './KanbanBadge';
import type { KanbanTag } from './KanbanCardContent';

/** Measure only on tag changes or available-width changes; never poll cards. */
export function KanbanTags({ tags }: { tags: KanbanTag[] }) {
  const container = useRef<HTMLSpanElement>(null);
  const probes = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(0);
  const signature = JSON.stringify(tags);

  useLayoutEffect(() => {
    const root = container.current;
    const measure = probes.current;
    if (!root || !measure) return;
    let previousWidth = -1;
    const fit = () => {
      const width = root.getBoundingClientRect().width;
      if (width === previousWidth) return;
      previousWidth = width;
      const badges = Array.from(measure.children).slice(0, -1);
      const gap = parseFloat(getComputedStyle(root).columnGap) || 0;
      const remainder = measure.lastElementChild!.getBoundingClientRect().width;
      let used = 0;
      let count = 0;
      for (let i = 0; i < badges.length; i++) {
        used += badges[i].getBoundingClientRect().width + (i ? gap : 0);
        const reserve = i < badges.length - 1 ? gap + remainder : 0;
        if (used + reserve <= width) count = i + 1;
      }
      // A single overlong first tag truncates beside the reserved +N.
      setVisible(Math.max(badges.length ? 1 : 0, count));
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(root);
    return () => observer.disconnect();
  }, [signature]);

  const count = Math.min(visible, tags.length);
  return (
    <span
      ref={container}
      className="relative flex items-center gap-half min-w-0 w-full"
      data-testid="kanban-tags"
    >
      {tags.slice(0, count).map((tag) => (
        <KanbanBadge
          key={tag.id}
          {...tag}
          className="min-w-0 max-w-full shrink"
        />
      ))}
      {count < tags.length && (
        <span className="text-sm text-low shrink-0 whitespace-nowrap">
          +{tags.length - count}
        </span>
      )}
      <span
        aria-hidden="true"
        className="absolute inset-0 invisible pointer-events-none overflow-hidden"
      >
        <span ref={probes} aria-hidden="true" className="flex gap-half w-max">
          {tags.map((tag) => (
            <KanbanBadge key={tag.id} {...tag} className="shrink-0" />
          ))}
          <span className="text-sm text-low shrink-0">+{tags.length}</span>
        </span>
      </span>
    </span>
  );
}
