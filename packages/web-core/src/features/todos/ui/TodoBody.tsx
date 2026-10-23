import { useId, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/shared/lib/utils';

export function TodoBody({
  body,
  completed,
  disabled,
  onEdit,
}: {
  body: string;
  completed: boolean;
  disabled: boolean;
  onEdit: () => void;
}) {
  const { t } = useTranslation('common');
  const id = useId();
  const probe = useRef<HTMLSpanElement>(null);
  const [overflow, setOverflow] = useState(false);
  const [expanded, setExpanded] = useState(false);

  useLayoutEffect(() => {
    const element = probe.current;
    if (!element) return;
    let active = true;
    const measure = () => {
      if (active) setOverflow(element.scrollHeight > element.clientHeight);
    };
    // This probe stays clamped even while the visible body is expanded.
    // It shares the body's width and typography, including whitespace handling.
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    void document.fonts.ready.then(measure);
    document.fonts.addEventListener('loadingdone', measure);
    return () => {
      active = false;
      observer.disconnect();
      document.fonts.removeEventListener('loadingdone', measure);
    };
  }, [body]);

  const textStyle = 'whitespace-pre-wrap [overflow-wrap:anywhere]';
  return (
    <div className="relative min-w-0">
      <span
        ref={probe}
        aria-hidden="true"
        className={cn(
          textStyle,
          'pointer-events-none invisible absolute inset-x-0 top-0 line-clamp-4'
        )}
      >
        {body}
      </span>
      <button
        type="button"
        disabled={disabled}
        onClick={(event) => {
          const selection = window.getSelection();
          if (
            event.detail > 0 &&
            selection &&
            !selection.isCollapsed &&
            selection.containsNode(event.currentTarget, true)
          )
            return;
          onEdit();
        }}
        className={cn(
          'block w-full text-left hover:text-high focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand',
          completed && 'text-low line-through'
        )}
      >
        <span
          id={id}
          className={cn(textStyle, expanded ? 'block' : 'line-clamp-4')}
        >
          {body}
        </span>
      </button>
      {overflow && (
        <button
          type="button"
          aria-expanded={expanded}
          aria-controls={id}
          onClick={() => setExpanded((value) => !value)}
          className="mt-half text-xs text-brand hover:underline focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand"
        >
          {t(expanded ? 'todo.collapseBody' : 'todo.expandBody')}
        </button>
      )}
    </div>
  );
}
