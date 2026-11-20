import { Fragment, memo, type CSSProperties } from "react";
import { useShikiCode } from "../lib/shiki-code";

/** Only decorates text; copy, whitespace, wrapping and layout belong to the caller. */
export const ChatCode = memo(function ChatCode({
  code,
  language,
}: {
  code: string;
  language?: string;
}) {
  const lines = useShikiCode(code, language);
  if (!lines) return <>{code}</>;
  const separators = code.match(/\r\n|\r|\n/g) ?? [];
  return (
    <>
      {lines.map((line, index) => (
        <Fragment key={index}>
          {index > 0 && separators[index - 1]}
          {line.map((token, tokenIndex) => (
            <span
              key={tokenIndex}
              className="shiki-token"
              style={token.htmlStyle as CSSProperties}
            >
              {token.content}
            </span>
          ))}
        </Fragment>
      ))}
    </>
  );
});
