import {
  canHighlightChatCode,
  tokenizeChatCode,
} from '@vibe/ui/lib/chat-code-highlighting';

export interface CodeLineSegment {
  text: string;
  types: string[];
}

// In addition to the shared total-size limit, avoid minified lines and an
// excessive number of React spans. The full plain text remains available.
export const MAX_FILE_HIGHLIGHT_LINE_LENGTH = 4096;
const MAX_SEGMENTS = 4096;
const MAX_TOKEN_DEPTH = 32;

type Token = ReturnType<typeof tokenizeChatCode>[number];

export function codeHighlightLines(
  content: string,
  language?: string | null
): (string | CodeLineSegment[])[] {
  const plain = content.split(/\r\n|\r|\n/);
  if (
    content.includes('\0') ||
    !canHighlightChatCode(content, language ?? undefined) ||
    plain.some((text) => text.length > MAX_FILE_HIGHLIGHT_LINE_LENGTH)
  ) {
    return plain;
  }

  try {
    const lines: CodeLineSegment[][] = [[]];
    let segments = 0;
    const append = (token: Token, types: string[]) => {
      if (++segments > MAX_SEGMENTS || types.length > MAX_TOKEN_DEPTH) {
        throw new Error('Highlight budget exceeded');
      }
      if (typeof token !== 'string') {
        const nestedTypes = [...types, token.type];
        if (Array.isArray(token.content)) {
          token.content.forEach((child) => append(child, nestedTypes));
        } else {
          append(token.content, nestedTypes);
        }
        return;
      }
      token.split('\n').forEach((text, index) => {
        if (index > 0) lines.push([]);
        if (text) {
          if (++segments > MAX_SEGMENTS)
            throw new Error('Highlight budget exceeded');
          lines[lines.length - 1].push({ text, types });
        }
      });
    };
    // Tokenize the entire document before splitting: multiline comments,
    // templates and nested tokens must retain their grammar across rows.
    const tokens = tokenizeChatCode(
      content.replace(/\r\n|\r/g, '\n'),
      language ?? undefined
    );
    if (tokens.every((token) => typeof token === 'string')) return plain;
    tokens.forEach((token) => append(token, []));
    return lines;
  } catch {
    return plain;
  }
}
