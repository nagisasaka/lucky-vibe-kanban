import { isMap, parseDocument } from 'yaml';

/** Only remove a complete, valid metadata mapping at the start of a file. */
export function splitMarkdownFrontmatter(content: string): {
  body: string;
  metadata: string | null;
} {
  const unchanged = { body: content, metadata: null };
  const opening = /^\uFEFF?---[\t ]*\r?\n/.exec(content);
  if (!opening) return unchanged;

  const rest = content.slice(opening[0].length);
  const closing = /^(?:---|\.\.\.)[\t ]*(?:\r?\n|$)/m.exec(rest);
  if (!closing) return unchanged;

  const metadata = rest.slice(0, closing.index);
  try {
    // Inspect the syntax tree only: do not resolve aliases or construct objects.
    const document = parseDocument(metadata, { schema: 'core' });
    if (document.errors.length || !isMap(document.contents)) return unchanged;
  } catch {
    return unchanged;
  }
  return {
    body: rest.slice(closing.index + closing[0].length),
    metadata,
  };
}
