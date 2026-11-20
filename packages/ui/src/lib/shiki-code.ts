import { useEffect, useState } from "react";
import type { ThemedToken } from "shiki";

export const MAX_HIGHLIGHT_LENGTH = 16_384;
const MAX_LINE_LENGTH = 4096;
type Result = { code: string; language: string; lines: ThemedToken[][] };
const cache = new Map<string, ThemedToken[][]>();
const MAX_CACHE_ENTRIES = 32;
const cacheKey = (code: string, language: string) =>
  JSON.stringify([language, code]);
let highlighterPromise: ReturnType<typeof createHighlighter> | undefined;

async function createHighlighter() {
  const [
    { createHighlighterCore },
    { createJavaScriptRegexEngine },
    { bundledThemes },
  ] = await Promise.all([
    import("shiki/core"),
    import("shiki/engine/javascript"),
    import("shiki/themes"),
  ]);
  return createHighlighterCore({
    engine: createJavaScriptRegexEngine(),
    langs: [],
    themes: [
      bundledThemes["catppuccin-latte"](),
      bundledThemes["catppuccin-mocha"](),
    ],
  });
}

/** Return null for unsupported, expensive or failed inputs so callers retain the source. */
export async function highlightCode(
  code: string,
  language?: string | null,
): Promise<ThemedToken[][] | null> {
  if (!language || code.length > MAX_HIGHLIGHT_LENGTH || code.includes("\0"))
    return null;
  const normalized = code.replace(/\r\n?/g, "\n");
  if (normalized.split("\n").some((line) => line.length > MAX_LINE_LENGTH))
    return null;
  try {
    const key = cacheKey(code, language);
    const cached = cache.get(key);
    if (cached) return cached;
    const { bundledLanguages } = await import("shiki/langs");
    if (!Object.hasOwn(bundledLanguages, language)) return null;
    const highlighter = await (highlighterPromise ??= createHighlighter());
    if (!highlighter.getLoadedLanguages().includes(language)) {
      await highlighter.loadLanguage(
        bundledLanguages[language as keyof typeof bundledLanguages](),
      );
    }
    const { tokens } = highlighter.codeToTokens(normalized, {
      lang: language,
      themes: { light: "catppuccin-latte", dark: "catppuccin-mocha" },
      defaultColor: false,
    });
    if (cache.size >= MAX_CACHE_ENTRIES)
      cache.delete(cache.keys().next().value!);
    cache.set(key, tokens);
    return tokens;
  } catch {
    highlighterPromise = undefined;
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
  return (
    (language && code.length <= MAX_HIGHLIGHT_LENGTH
      ? cache.get(cacheKey(code, language))
      : undefined) ??
    (result?.code === code && result.language === language
      ? result.lines
      : null)
  );
}
