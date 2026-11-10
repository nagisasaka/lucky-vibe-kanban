import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

const remarkPlugins = [remarkGfm];
const allowedElements = ['strong', 'em', 'code', 'del'];

/** Non-interactive inline text for disclosure headings. Never mounts a body editor. */
export function MarkdownInline({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={remarkPlugins}
      allowedElements={allowedElements}
      unwrapDisallowed
      skipHtml
    >
      {content}
    </ReactMarkdown>
  );
}
