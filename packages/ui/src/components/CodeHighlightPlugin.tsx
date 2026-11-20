import { useLayoutEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $createCodeHighlightNode,
  PrismTokenizer,
  registerCodeHighlighting,
} from '@lexical/code';
import { $createLineBreakNode, $createTabNode } from 'lexical';
import { canHighlightChatCode } from '../lib/chat-code-highlighting';

export const readOnlyTokenizer: typeof PrismTokenizer = {
  ...PrismTokenizer,
  defaultLanguage: 'javascript',
  $tokenize(node, language) {
    const code = node.getTextContent();
    const highlightLanguage = !language
      ? 'javascript'
      : language === 'diff'
        ? 'diff-javascript'
        : language;
    const grammar = highlightLanguage.startsWith('diff-')
      ? highlightLanguage.slice(5)
      : highlightLanguage;
    if (canHighlightChatCode(code, grammar)) {
      try {
        return PrismTokenizer.$tokenize(node, highlightLanguage);
      } catch {
        // Incomplete or unsupported code must remain readable during streaming.
      }
    }
    return code.split(/(\n|\t)/).map((part) => {
      if (part === '\n') return $createLineBreakNode();
      if (part === '\t') return $createTabNode();
      return $createCodeHighlightNode(part);
    });
  },
};

export function CodeHighlightPlugin({
  readOnly = false,
}: {
  readOnly?: boolean;
}) {
  const [editor] = useLexicalComposerContext();

  // Register transforms before MarkdownSyncPlugin parses the initial history.
  // Late registration can leave the first render plain until a stream update.
  useLayoutEffect(() => {
    return registerCodeHighlighting(
      editor,
      readOnly ? readOnlyTokenizer : undefined
    );
  }, [editor, readOnly]);

  return null;
}
