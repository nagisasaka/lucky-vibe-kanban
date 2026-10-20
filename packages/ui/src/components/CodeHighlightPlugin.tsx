import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $createCodeHighlightNode,
  PrismTokenizer,
  registerCodeHighlighting,
} from '@lexical/code';
import { $createLineBreakNode, $createTabNode } from 'lexical';
import { canHighlightChatCode } from '../lib/chat-code-highlighting';

const readOnlyTokenizer: typeof PrismTokenizer = {
  ...PrismTokenizer,
  defaultLanguage: 'text',
  $tokenize(node, language) {
    const code = node.getTextContent();
    if (canHighlightChatCode(code, language)) {
      try {
        return PrismTokenizer.$tokenize(node, language);
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

  useEffect(() => {
    return registerCodeHighlighting(
      editor,
      readOnly ? readOnlyTokenizer : undefined
    );
  }, [editor, readOnly]);

  return null;
}
