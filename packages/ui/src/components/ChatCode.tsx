import { memo, useMemo, type ReactNode } from 'react';
import { tokenizeChatCode } from '../lib/chat-code-highlighting';
import { CODE_HIGHLIGHT_CLASSES } from '../lib/code-highlight-theme';

type Token = ReturnType<typeof tokenizeChatCode>[number];

function renderToken(token: Token, key: number): ReactNode {
  if (typeof token === 'string') return token;
  const children = Array.isArray(token.content)
    ? token.content.map(renderToken)
    : renderToken(token.content, 0);
  return (
    <span key={key} className={CODE_HIGHLIGHT_CLASSES[token.type]}>
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
  return useMemo(
    () => <>{tokenizeChatCode(code, language).map(renderToken)}</>,
    [code, language]
  );
});
