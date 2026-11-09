import {
  useMemo,
  type ComponentProps,
  type ComponentPropsWithoutRef,
} from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';
import { cn } from '@/shared/lib/utils';
import { MarkdownReadingSurface } from '@vibe/ui/components/MarkdownReadingSurface';
import { MermaidDiagram } from './MermaidDiagram';

interface MarkdownPreviewProps {
  content: string;
  theme: 'light' | 'dark';
  className?: string;
  disableImages?: boolean;
  outline?: boolean;
  contentKey?: string;
  density?: 'document' | 'compact' | 'github';
}

const remarkPlugins = [remarkGfm];
const rehypePlugins: NonNullable<
  ComponentProps<typeof ReactMarkdown>['rehypePlugins']
> = [rehypeRaw, [rehypeSanitize, defaultSchema], rehypeHighlight];

export function MarkdownPreview({
  content,
  theme,
  className,
  disableImages = false,
  outline = false,
  contentKey = content,
  density = 'document',
}: MarkdownPreviewProps) {
  const components = useMemo(
    () => ({
      // Keep the marker as text for copying and non-supporting renderers.
      p: ({ children, ...props }: ComponentPropsWithoutRef<'p'>) => {
        const parts = Array.isArray(children) ? children : [children];
        const first = parts[0];
        const marker =
          typeof first === 'string' &&
          /^(\[!(?:DECISION|NOTE|TIP|IMPORTANT|WARNING|CAUTION)\])(?:\n|$)/.exec(
            first
          );
        return (
          <p {...props}>
            {marker ? (
              <>
                <span>{marker[0]}</span>
                {first.slice(marker[0].length)}
                {parts.slice(1)}
              </>
            ) : (
              children
            )}
          </p>
        );
      },
      a: ({ children, ...props }: ComponentPropsWithoutRef<'a'>) => (
        <a {...props} target="_blank" rel="noopener noreferrer">
          {children}
        </a>
      ),
      img: (props: ComponentPropsWithoutRef<'img'>) =>
        disableImages ? null : <img {...props} alt={props.alt ?? ''} />,
      pre: ({ children, ...props }: ComponentPropsWithoutRef<'pre'>) => {
        // When a mermaid code block is rendered, the code component returns
        // <MermaidDiagram> but react-markdown still wraps it in <pre>.
        // Detect this and render a plain wrapper instead.
        const child = Array.isArray(children) ? children[0] : children;
        if (
          child &&
          typeof child === 'object' &&
          'props' in child &&
          child.props?.className &&
          /language-mermaid/.test(child.props.className)
        ) {
          return <>{children}</>;
        }
        return (
          <pre className="markdown-code-block" {...props}>
            {children}
          </pre>
        );
      },
      code: ({
        className: codeClassName,
        children,
        ...props
      }: ComponentPropsWithoutRef<'code'>) => {
        const match = /language-(\w+)/.exec(codeClassName || '');
        const language = match?.[1];
        const codeString = String(children).replace(/\n$/, '');

        // Mermaid diagrams
        if (language === 'mermaid') {
          return <MermaidDiagram chart={codeString} theme={theme} />;
        }

        // All code elements: fenced blocks get styling from the <pre> wrapper,
        // inline code gets styling via the wrapper div's CSS selector.
        return (
          <code className={codeClassName} {...props}>
            {children}
          </code>
        );
      },
      input: ({
        type,
        checked,
        ...props
      }: ComponentPropsWithoutRef<'input'>) => {
        if (type === 'checkbox') {
          return (
            <input type="checkbox" checked={checked} readOnly {...props} />
          );
        }
        return <input type={type} checked={checked} {...props} />;
      },
    }),
    [theme, disableImages]
  );

  return (
    <MarkdownReadingSurface contentKey={contentKey} outline={outline}>
      <div
        className={cn(
          'markdown-preview markdown-body',
          `markdown-body--${density}`,
          className
        )}
      >
        <ReactMarkdown
          remarkPlugins={remarkPlugins}
          rehypePlugins={rehypePlugins}
          components={components}
        >
          {content}
        </ReactMarkdown>
      </div>
    </MarkdownReadingSurface>
  );
}
