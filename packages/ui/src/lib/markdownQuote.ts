import { QUOTE } from '@lexical/markdown';

// CommonMark also allows a bare `>` for a blank quoted line. Lexical's default
// requires whitespace, which otherwise renders that separator as literal text.
export const MARKDOWN_QUOTE = { ...QUOTE, regExp: /^>\s?/ };
