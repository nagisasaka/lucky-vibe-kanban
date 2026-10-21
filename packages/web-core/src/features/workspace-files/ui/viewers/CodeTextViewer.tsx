import { useEffect, useRef } from 'react';
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
  const lines = content.length === 0 ? [''] : content.split(/\r\n|\r|\n/);
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
            <code className="whitespace-pre px-base text-normal">{line}</code>
          </div>
        ))}
      </div>
    </div>
  );
}
