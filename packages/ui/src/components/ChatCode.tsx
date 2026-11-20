import { memo, useMemo, type ReactNode } from 'react';
import {
  canHighlightChatCode,
  tokenizeChatCode,
} from '../lib/chat-code-highlighting';
import { CODE_HIGHLIGHT_CLASSES } from '../lib/code-highlight-theme';

type Token = ReturnType<typeof tokenizeChatCode>[number];

type Range = { start: number; end: number };

function tokenText(token: Token): string {
  if (typeof token === 'string') return token;
  return Array.isArray(token.content)
    ? token.content.map(tokenText).join('')
    : tokenText(token.content);
}

// Use Prism's shell operators to avoid treating quoted text or comments as
// redirections. Only mark delimiters; the here-document body stays untouched.
function heredocRanges(code: string, tokens: Token[]): Range[] {
  const ranges: Range[] = [];
  const operators: { start: number; end: number; value: string }[] = [];
  function collect(token: Token, start: number, shellContext: boolean) {
    if (typeof token === 'string') return;
    const value = tokenText(token);
    if (shellContext && token.type === 'operator' && /^<<-?$/.test(value)) {
      operators.push({ start, end: start + value.length, value });
    }
    if (!Array.isArray(token.content)) return;
    let childStart = start;
    for (const child of token.content) {
      // Prism nests commands following the first delimiter in a bash token.
      const childShell = shellContext && token.type === 'bash';
      if (childShell || (typeof child !== 'string' && child.type === 'bash')) {
        collect(child, childStart, childShell || token.type === 'string');
      }
      childStart += tokenText(child).length;
    }
  }
  let offset = 0;
  for (const token of tokens) {
    collect(token, offset, true);
    offset += tokenText(token).length;
  }
  const bodies: Range[] = [];
  for (const { start, end: operatorEnd, value } of operators) {
    if (bodies.some((body) => start >= body.start && start < body.end))
      continue;

    const remainder = code.slice(operatorEnd);
    const opener = /^[ \t]*(?:(['"])(\w+)\1|(\w+))(?=$|[\s;|&<>])/.exec(
      remainder
    );
    if (!opener) continue;
    const label = opener[2] ?? opener[3];
    const leading = /^[ \t]*/.exec(opener[0])![0].length;
    ranges.push({
      start: operatorEnd + leading,
      end: operatorEnd + opener[0].length,
    });

    const lineEnd = code.indexOf('\n', operatorEnd + opener[0].length);
    if (lineEnd < 0) continue;
    const closing = new RegExp(
      `^${value === '<<-' ? '\\t*' : ''}${label}(?=\\r?$)`,
      'm'
    );
    const match = closing.exec(code.slice(lineEnd + 1));
    if (match) {
      const end = lineEnd + 1 + match.index + match[0].length;
      ranges.push({ start: end - label.length, end });
      bodies.push({ start: lineEnd + 1, end });
    }
  }
  return ranges.sort((a, b) => a.start - b.start);
}

function renderToken(
  token: Token,
  key: number,
  shell: boolean,
  ranges: Range[],
  cursor: { offset: number }
): ReactNode {
  if (typeof token === 'string') {
    const start = cursor.offset;
    cursor.offset += token.length;
    const parts: ReactNode[] = [];
    let position = start;
    for (const range of ranges) {
      const from = Math.max(start, range.start);
      const to = Math.min(cursor.offset, range.end);
      if (from >= to) continue;
      if (position < from)
        parts.push(token.slice(position - start, from - start));
      parts.push(
        <span key={from} className={CODE_HIGHLIGHT_CLASSES.keyword}>
          {token.slice(from - start, to - start)}
        </span>
      );
      position = to;
    }
    if (position < cursor.offset) parts.push(token.slice(position - start));
    return parts.length ? parts : token;
  }
  const children = Array.isArray(token.content)
    ? token.content.map((child, index) =>
        renderToken(child, index, shell, ranges, cursor)
      )
    : renderToken(token.content, 0, shell, ranges, cursor);
  return (
    <span
      key={key}
      className={
        shell
          ? (
              {
                function: CODE_HIGHLIGHT_CLASSES.function,
                builtin: CODE_HIGHLIGHT_CLASSES.function,
                parameter: CODE_HIGHLIGHT_CLASSES.variable,
                operator: CODE_HIGHLIGHT_CLASSES.operator,
              } as Record<string, string>
            )[token.type]
          : CODE_HIGHLIGHT_CLASSES[token.type]
      }
    >
      {children}
    </span>
  );
}

/** Only decorates text; copy, whitespace, wrapping and layout belong to the caller. */
export const ChatCode = memo(function ChatCode({
  code,
  language,
}: {
  code: string;
  language?: string;
}) {
  return useMemo(() => {
    const tokens = tokenizeChatCode(code, language);
    const shell = language === 'bash';
    const ranges =
      shell && canHighlightChatCode(code, language)
        ? heredocRanges(code, tokens)
        : [];
    const cursor = { offset: 0 };
    return (
      <>
        {tokens.map((token, index) =>
          renderToken(token, index, shell, ranges, cursor)
        )}
      </>
    );
  }, [code, language]);
});
