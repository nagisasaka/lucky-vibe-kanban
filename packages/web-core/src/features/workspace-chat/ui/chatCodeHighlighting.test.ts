import { describe, expect, it, vi } from 'vitest';
import {
  CodeHighlightNode,
  CodeNode,
  PrismTokenizer,
  registerCodeHighlighting,
} from '@lexical/code';
import { $convertFromMarkdownString, CODE } from '@lexical/markdown';
import { $getRoot, createEditor } from 'lexical';
import { readOnlyTokenizer } from '@vibe/ui/components/CodeHighlightPlugin';
import {
  MAX_CHAT_HIGHLIGHT_LENGTH,
  tokenizeChatCode,
} from '@vibe/ui/lib/chat-code-highlighting';

type Token = ReturnType<typeof tokenizeChatCode>[number];
function flatten(token: Token): string {
  if (typeof token === 'string') return token;
  return Array.isArray(token.content)
    ? token.content.map(flatten).join('')
    : flatten(token.content);
}
function types(token: Token): string[] {
  if (typeof token === 'string') return [];
  return [
    token.type,
    ...(Array.isArray(token.content)
      ? token.content.flatMap(types)
      : types(token.content)),
  ];
}

describe('chat code highlighting', () => {
  it('keeps oversized fenced code intact without tokenizing', () => {
    const source = 'return x;\n'.repeat(MAX_CHAT_HIGHLIGHT_LENGTH);
    const editor = createEditor({
      namespace: 'test',
      nodes: [CodeNode, CodeHighlightNode],
      onError: (error) => {
        throw error;
      },
    });
    const spy = vi.spyOn(PrismTokenizer, '$tokenize');
    const unregister = registerCodeHighlighting(editor, readOnlyTokenizer);
    try {
      editor.update(
        () => $convertFromMarkdownString(`\`\`\`\n${source}\`\`\``, [CODE]),
        { discrete: true }
      );
      editor
        .getEditorState()
        .read(() => expect($getRoot().getTextContent()).toBe(source.trimEnd()));
      expect(spy).not.toHaveBeenCalled();
    } finally {
      unregister();
      spy.mockRestore();
    }
  });
  it.each([
    ['', 'return value;', 'return', 'keyword'],
    [
      'diff',
      '--- a/file\n+++ b/file\n@@ -1 +1 @@\n-return old;\n+return next;',
      'return',
      'keyword',
    ],
    ['diff-python', '-return old\n+return True', 'True', 'boolean'],
    ['tsx', 'export const Label = () => <span />;', 'span', 'tag'],
    ['text', 'return value;', 'return', undefined],
    ['plain', 'return value;', 'return', undefined],
  ])(
    'renders fenced %s with its intended grammar',
    (language, source, word, expected) => {
      const editor = createEditor({
        namespace: 'test',
        nodes: [CodeNode, CodeHighlightNode],
        onError: (error) => {
          throw error;
        },
      });
      const unregister = registerCodeHighlighting(editor, readOnlyTokenizer);
      try {
        editor.update(
          () =>
            $convertFromMarkdownString(`\`\`\`${language}\n${source}\n\`\`\``, [
              CODE,
            ]),
          { discrete: true }
        );
        editor.getEditorState().read(() => {
          const code = $getRoot().getFirstChildOrThrow() as CodeNode;
          expect(code.getTextContent()).toBe(source);
          const nodes = code.getChildren();
          const matching = nodes.filter((node) =>
            node.getTextContent().includes(word)
          );
          expect(
            matching.some(
              (node) =>
                node instanceof CodeHighlightNode &&
                node.getHighlightType() === expected
            )
          ).toBe(true);
          if (language === 'diff') {
            const highlights = nodes.filter(
              (node): node is CodeHighlightNode =>
                node instanceof CodeHighlightNode
            );
            expect(
              highlights.some(
                (node) =>
                  node.getTextContent() === '+' &&
                  node.getHighlightType() === 'inserted'
              )
            ).toBe(true);
            expect(
              highlights.some(
                (node) =>
                  node.getTextContent() === '-' &&
                  node.getHighlightType() === 'deleted'
              )
            ).toBe(true);
            expect(
              highlights.some(
                (node) =>
                  node.getTextContent().startsWith('---') &&
                  node.getHighlightType() === 'coord'
              )
            ).toBe(true);
          }
        });
      } finally {
        unregister();
      }
    }
  );
  it('highlights a TSX Markdown code block through Lexical', () => {
    const source =
      'export function SearchLabel() { return <span>Search</span>; }';
    const editor = createEditor({
      namespace: 'test',
      nodes: [CodeNode, CodeHighlightNode],
      onError: (error) => {
        throw error;
      },
    });
    const unregister = registerCodeHighlighting(editor);
    try {
      editor.update(
        () =>
          $convertFromMarkdownString(`\`\`\`tsx\n${source}\n\`\`\``, [CODE]),
        { discrete: true }
      );
      editor.getEditorState().read(() => {
        const code = $getRoot().getFirstChildOrThrow() as CodeNode;
        expect(code.getLanguage()).toBe('tsx');
        expect(code.getTextContent()).toBe(source);
        const highlights = code
          .getChildren()
          .map((node) =>
            node instanceof CodeHighlightNode
              ? node.getHighlightType()
              : undefined
          );
        expect(highlights).toContain('keyword');
        expect(highlights).toContain('tag');
      });
    } finally {
      unregister();
    }
  });
  it.each([
    [
      'tsx',
      'export function SearchLabel() { return <span>Search</span>; }',
      ['keyword', 'tag'],
    ],
    [
      'jsx',
      'export const Label = () => <span>Search</span>;',
      ['keyword', 'tag'],
    ],
    ['typescript', 'const answer: number = 42;', ['keyword']],
    ['javascript', 'const answer = true;', ['keyword']],
    ['json', '{"answer": 42}', ['property']],
  ])(
    'highlights %s while preserving the source',
    (language, source, expected) => {
      const tokens = tokenizeChatCode(source, language);
      expect(tokens.map(flatten).join('')).toBe(source);
      for (const type of expected)
        expect(tokens.flatMap(types)).toContain(type);
    }
  );

  it.each([undefined, 'unknown-language'])(
    'keeps %s code plain',
    (language) => {
      const source = 'export <unfinished\n\ttext';
      expect(tokenizeChatCode(source, language)).toEqual([source]);
    }
  );

  it('preserves diff text', () => {
    const source = '-old line\n+new line';
    expect(tokenizeChatCode(source, 'diff').map(flatten).join('')).toBe(source);
  });

  it('keeps oversized and incomplete TSX readable', () => {
    const long = 'x'.repeat(MAX_CHAT_HIGHLIGHT_LENGTH + 1);
    expect(tokenizeChatCode(long, 'tsx')).toEqual([long]);
    const partial = 'export function SearchLabel() { return <span';
    expect(tokenizeChatCode(partial, 'tsx').map(flatten).join('')).toBe(
      partial
    );
  });

  it('keeps the original text if tokenization fails', () => {
    const spy = vi
      .spyOn(PrismTokenizer, 'tokenize')
      .mockImplementationOnce(() => {
        throw new Error('grammar failure');
      });
    try {
      expect(tokenizeChatCode('export <span />', 'tsx')).toEqual([
        'export <span />',
      ]);
    } finally {
      spy.mockRestore();
    }
  });
});
