import { MarkdownReadingSurface } from '@vibe/ui/components/MarkdownReadingSurface';

/** Archival text never resolves source links, images, paths or attachments. */
export function TransferArchiveText({ content }: { content: string }) {
  return (
    <MarkdownReadingSurface contentKey={content} readOnly>
      <pre className="whitespace-pre-wrap break-words font-ibm-plex-sans">
        {content}
      </pre>
    </MarkdownReadingSurface>
  );
}
