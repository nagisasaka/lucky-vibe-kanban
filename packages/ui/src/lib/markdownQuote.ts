import { QUOTE } from '@lexical/markdown';

// CommonMark also allows a bare `>` for a blank quoted line. Lexical's default
// requires whitespace, which otherwise renders that separator as literal text.
export const MARKDOWN_QUOTE: typeof QUOTE = {
  ...QUOTE,
  regExp: /^>\s?/,
  export: (node, exportChildren, exportFormat) => {
    const output = QUOTE.export?.(node, exportChildren, exportFormat);
    // Lexical 0.53 escapes literal brackets, but our alert syntax requires a
    // standalone marker at the beginning of a quote to remain recognizable.
    return output?.replace(
      /^> &#91;!(DECISION|NOTE|TIP|IMPORTANT|WARNING|CAUTION)&#93;(?=\n|$)/,
      '> [!$1]'
    ) ?? null;
  },
};
