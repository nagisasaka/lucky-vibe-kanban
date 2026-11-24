import { unified } from 'unified';
import { parseFragment, type DefaultTreeAdapterMap } from 'parse5';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';

const parser = unified().use(remarkParse).use(remarkGfm);

type SpeechNode = {
  type: string;
  value?: string;
  alt?: string | null;
  children?: SpeechNode[];
};

function htmlText(node: DefaultTreeAdapterMap['node']): string {
  if ('value' in node) return node.value;
  if (['script', 'style', 'template'].includes(node.nodeName)) return ' ';
  if ('childNodes' in node)
    return ` ${node.childNodes.map(htmlText).join('')} `;
  return ' ';
}

function plainText(node: SpeechNode): string {
  switch (node.type) {
    case 'text':
      return node.value ?? '';
    case 'inlineCode':
    case 'code':
      // Code is literal: never parse its operators as Markdown.
      return ` ${node.value ?? ''} `;
    case 'image':
    case 'imageReference':
      return ` ${node.alt ?? ''} `;
    case 'html':
      return htmlText(parseFragment(node.value ?? ''));
    case 'definition':
    case 'thematicBreak':
    case 'break':
      return ' ';
    default: {
      const content = (node.children ?? []).map(plainText).join('');
      // Markup and block boundaries must not concatenate adjacent words.
      return ` ${content} `;
    }
  }
}

/** Only for speech. The stored and displayed Markdown remains untouched. */
export function speechText(markdown: string): string {
  return plainText(parser.parse(markdown)).replace(/\s+/gu, ' ').trim();
}
