import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';

const ALERTS = {
  DECISION: ['decision', '◇'],
  NOTE: ['note', 'ⓘ'],
  TIP: ['tip', '✧'],
  IMPORTANT: ['important', '❖'],
  WARNING: ['warning', '⚠'],
  CAUTION: ['caution', '⛔'],
} as const;

type Heading = { element: HTMLElement; label: string; level: number };

/** Display-only DOM metadata. Never updates Lexical state or serialized Markdown. */
export function MarkdownReadingSurface({
  children,
  contentKey,
  outline = false,
  readOnly = true,
  className,
}: {
  children: ReactNode;
  contentKey: string;
  outline?: boolean;
  readOnly?: boolean;
  className?: string;
}) {
  const { t } = useTranslation('common');
  const rootRef = useRef<HTMLDivElement>(null);
  const [headings, setHeadings] = useState<Heading[]>([]);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    setOpen(false);
    const root = rootRef.current;
    if (!root) return;
    const clearAlerts = () => {
      root.querySelectorAll('[data-markdown-alert]').forEach((quote) => {
        quote.removeAttribute('data-markdown-alert');
        quote.removeAttribute('data-alert-label');
      });
      root.querySelectorAll('[data-alert-marker]').forEach((marker) => {
        marker.removeAttribute('data-alert-marker');
      });
    };
    if (!readOnly) {
      clearAlerts();
      setHeadings([]);
      return;
    }
    const refresh = () => {
      clearAlerts();
      root.querySelectorAll('blockquote').forEach((quote) => {
        // Only an explicit, standalone marker at the start of a quote is special.
        const match =
          /^\[!(DECISION|NOTE|TIP|IMPORTANT|WARNING|CAUTION)\](?:\r?\n|$)/.exec(
            (quote as HTMLElement).innerText ?? ''
          );
        if (!match) return;
        const type = match[1] as keyof typeof ALERTS;
        const [name, icon] = ALERTS[type];
        quote.setAttribute('data-markdown-alert', name);
        quote.setAttribute(
          'data-alert-label',
          `${icon} ${t(`markdownReading.${name}`)}`
        );
        // Lexical has a leaf span; react-markdown has an explicit marker span.
        // Hide only the marker leaf, never a paragraph containing the message.
        const marker = Array.from(quote.querySelectorAll('span')).find(
          (span) => span.textContent?.trim() === match[0].trim()
        );
        marker?.setAttribute('data-alert-marker', 'true');
      });
      if (!outline) return;
      const next = Array.from(
        root.querySelectorAll<HTMLElement>(
          '.markdown-body h1, .markdown-body h2, .markdown-body h3'
        )
      )
        .filter((element) => {
          let parent: HTMLElement | null = element.parentElement;
          while (parent && parent !== root) {
            if (parent instanceof HTMLDetailsElement && !parent.open)
              return false;
            parent = parent.parentElement;
          }
          return true;
        })
        .map((element) => ({
          element,
          label: element.textContent ?? '',
          level: Number(element.tagName[1]),
        }));
      setHeadings((previous) =>
        previous.length === next.length &&
        previous.every(
          (h, i) =>
            h.element === next[i].element &&
            h.label === next[i].label &&
            h.level === next[i].level
        )
          ? previous
          : next
      );
    };
    refresh();
    const observer = new MutationObserver(refresh);
    observer.observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    root.addEventListener('toggle', refresh, true);
    return () => {
      observer.disconnect();
      root.removeEventListener('toggle', refresh, true);
      clearAlerts();
    };
  }, [contentKey, readOnly, outline, t]);

  const navigate = ({ element }: Heading) => {
    if (!rootRef.current?.contains(element)) return;
    // scrollIntoView can also scroll ancestors and the browser viewport.
    let pane = rootRef.current.parentElement;
    while (pane) {
      if (/(auto|scroll)/.test(getComputedStyle(pane).overflowY)) break;
      pane = pane.parentElement;
    }
    if (pane) {
      pane.scrollTop +=
        element.getBoundingClientRect().top -
        pane.getBoundingClientRect().top -
        12;
    }
    element.tabIndex = -1;
    element.focus({ preventScroll: true });
  };

  return (
    <div
      ref={rootRef}
      className={cn('markdown-reading-surface min-w-0', className)}
    >
      {outline && readOnly && headings.length >= 3 && (
        <nav
          aria-label={t('markdownReading.outline')}
          className="markdown-outline"
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            <span aria-hidden="true">{open ? '▾' : '▸'}</span>{' '}
            {t('markdownReading.outline')}
          </button>
          {open && (
            <ol>
              {headings.map((heading, index) => (
                <li
                  key={index}
                  style={{ paddingInlineStart: `${heading.level - 1}em` }}
                >
                  <button type="button" onClick={() => navigate(heading)}>
                    {heading.label}
                  </button>
                </li>
              ))}
            </ol>
          )}
        </nav>
      )}
      {children}
    </div>
  );
}
