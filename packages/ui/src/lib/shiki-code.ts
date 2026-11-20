import { useEffect, useState } from 'react';
import type { ThemedToken } from 'shiki';

export const MAX_HIGHLIGHT_LENGTH = 16_384;
const MAX_LINE_LENGTH = 4096;
type Result = { code: string; language: string; lines: ThemedToken[][] };

/** Return null for unsupported, expensive or failed inputs so callers retain the source. */
export async function highlightCode(
  code: string,
  language?: string | null
): Promise<ThemedToken[][] | null> {
  if (
    !language ||
    code.length > MAX_HIGHLIGHT_LENGTH ||
    code.includes('\0') ||
    code.includes('\r') ||
    code.split('\n').some((line) => line.length > MAX_LINE_LENGTH)
  )
    return null;
  try {
    const { codeToTokens, bundledLanguages } = await import('shiki');
    if (!(language in bundledLanguages)) return null;
    const { tokens } = await codeToTokens(code, {
      lang: language as keyof typeof bundledLanguages,
      themes: { light: 'catppuccin-latte', dark: 'catppuccin-mocha' },
      defaultColor: false,
    });
    return tokens;
  } catch {
    return null;
  }
}

/** Keep original text visible while the grammar loads or if highlighting fails. */
export function useShikiCode(code: string, language?: string | null) {
  const [result, setResult] = useState<Result | null>(null);
  useEffect(() => {
    let cancelled = false;
    void highlightCode(code, language).then((lines) => {
      if (!cancelled)
        setResult(lines && language ? { code, language, lines } : null);
    });
    return () => {
      cancelled = true;
    };
  }, [code, language]);
  return result?.code === code && result.language === language
    ? result.lines
    : null;
}
