import { Fragment, useEffect, useMemo, useRef, type ReactNode } from 'react';
import { CODE_HIGHLIGHT_CLASSES } from '@vibe/ui/lib/code-highlight-theme';
import { codeHighlightLines } from '../../model/codeHighlightLines';
import { cn } from '@/shared/lib/utils';

interface CodeTextViewerProps {
  content: string;
  line?: number;
  language?: string | null;
  className?: string;
}

export function CodeTextViewer({
  content,
  language,
  line: selectedLine,
  className,
}: CodeTextViewerProps) {
  const selectedRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: 'center' });
  }, [content, selectedLine]);
  const lines = useMemo(
    () => codeHighlightLines(content, language),
    [content, language]
  );
  const lineNumberWidth = Math.max(2, String(lines.length).length);

  return (
    <div className={cn('h-full min-h-0 overflow-auto bg-primary', className)}>
      {language && (
        <div className="sticky top-0 z-[1] border-b border-border bg-primary px-base py-half text-xs uppercase text-low">
          {language}
        </div>
      )}
      <div className="min-w-max py-base font-ibm-plex-mono text-sm leading-5">
        {lines.map((line, index) => (
          <div
            key={index}
            ref={index + 1 === selectedLine ? selectedRef : undefined}
            aria-current={index + 1 === selectedLine ? 'location' : undefined}
            className={cn(
              'grid min-h-5 grid-cols-[auto_1fr] hover:bg-secondary/70',
              index + 1 === selectedLine && 'bg-secondary'
            )}
          >
            <span
              className="select-none border-r border-border px-base text-right text-low"
              style={{ width: `${lineNumberWidth + 3}ch` }}
            >
              {index + 1}
            </span>
            <code className="whitespace-pre px-base text-normal">
              {typeof line === 'string'
                ? line
                : line.map((segment, segmentIndex) => (
                    <Fragment key={segmentIndex}>
                      {segment.types.reduceRight<ReactNode>(
                        (children, type) => (
                          <span className={CODE_HIGHLIGHT_CLASSES[type]}>
                            {children}
                          </span>
                        ),
                        segment.text
                      )}
                    </Fragment>
                  ))}
            </code>
          </div>
        ))}
      </div>
    </div>
  );
}
