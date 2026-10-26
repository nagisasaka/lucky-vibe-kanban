import {
  $getRoot,
  $isElementNode,
  $createParagraphNode,
  $createTextNode,
  type LexicalNode,
} from 'lexical';
import type { TextMatchTransformer, Transformer } from '@lexical/markdown';

/** Literal chat text, with existing structured attachments exported by their codec. */
export function $getPlainText(transformers: Transformer[]): string {
  const read = (node: LexicalNode): string => {
    if ($isElementNode(node)) return node.getChildren().map(read).join('');
    for (const transformer of transformers) {
      if (transformer.type !== 'text-match' || !transformer.export) continue;
      const value = transformer.export(node, read, (_node, text) => text);
      if (value !== null) return value;
    }
    return node.getTextContent();
  };
  return $getRoot().getChildren().map(read).join('\n');
}

/** Restore literal text, recognising only the host's explicit attachment codecs. */
export function $setPlainText(
  value: string,
  attachments: readonly TextMatchTransformer[]
) {
  const paragraph = $createParagraphNode();
  $getRoot().clear().append(paragraph);
  let remaining = value;
  while (remaining) {
    const next = attachments
      .map((transformer) => ({
        transformer,
        match: transformer.importRegExp?.exec(remaining),
      }))
      .filter((candidate) => candidate.match && candidate.match[0].length > 0)
      .sort((a, b) => a.match!.index - b.match!.index)[0];
    if (!next?.match) {
      paragraph.append($createTextNode(remaining));
      break;
    }
    const { match, transformer } = next;
    if (match.index)
      paragraph.append($createTextNode(remaining.slice(0, match.index)));
    const text = $createTextNode(match[0]);
    paragraph.append(text);
    transformer.replace?.(text, match);
    remaining = remaining.slice(match.index + match[0].length);
  }
}
