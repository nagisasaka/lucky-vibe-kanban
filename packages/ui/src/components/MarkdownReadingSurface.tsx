import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { getMarkdownAlert } from '../lib/markdownAlert';

// Read authored line/paragraph boundaries, independent of CSS whitespace and
// whether a marker is currently hidden. This never mutates the displayed DOM.
function quoteText(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    // react-markdown inserts formatting whitespace between block children.
    if (node.parentNode?.nodeName === 'BLOCKQUOTE' && !node.textContent?.trim())
      return '';
    return node.textContent ?? '';
  }
  if (node.nodeName === 'BR') return '\n';
  const text = Array.from(node.childNodes, quoteText).join('');
  return node.nodeName === 'P' ? `${text}\n` : text;
}

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
        const alert = getMarkdownAlert(quoteText(quote));
        if (!alert) return;
        const { name, icon } = alert;
        quote.setAttribute('data-markdown-alert', name);
        quote.setAttribute(
          'data-alert-label',
          `${icon} ${t(`markdownReading.${name}`)}`
        );
        // Lexical has a leaf span; react-markdown has an explicit marker span.
        // Hide only the marker leaf, never a paragraph containing the message.
        const marker = Array.from(quote.querySelectorAll('span')).find(
          (span) => span.textContent?.trim() === alert.marker.trim()
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
