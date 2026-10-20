import { PrismTokenizer, getCodeLanguages } from '@lexical/code';
import 'prismjs/components/prism-bash';
import 'prismjs/components/prism-json';

// Bound synchronous tokenization during streaming. Keep the entire text visible.
export const MAX_CHAT_HIGHLIGHT_LENGTH = 16_384;

export function canHighlightChatCode(code: string, language?: string) {
  return (
    code.length <= MAX_CHAT_HIGHLIGHT_LENGTH &&
    !!language &&
    getCodeLanguages().includes(language)
  );
}

export function tokenizeChatCode(code: string, language?: string) {
  if (!canHighlightChatCode(code, language)) return [code];
  try {
    return PrismTokenizer.tokenize(code, language);
  } catch {
    return [code];
  }
}
