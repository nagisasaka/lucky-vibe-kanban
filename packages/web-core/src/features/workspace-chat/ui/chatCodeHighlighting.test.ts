import { describe, expect, it } from 'vitest';
import { highlightCode, MAX_HIGHLIGHT_LENGTH } from '@vibe/ui/lib/shiki-code';

describe('Shiki code highlighting', () => {
  it.each([
    ['tsx', 'export const View = () => <span>hello</span>;'],
    ['typescript', 'const answer: number = 42;'],
    ['rust', 'fn main() { println!("hello"); }'],
    ['python', 'def answer():\n    return 42'],
    ['bash', 'cat <<EOF\nhello\nEOF'],
    ['json', '{"answer": 42}'],
  ])(
    'preserves %s source and emits both theme colours',
    async (language, source) => {
      const lines = await highlightCode(source, language);
      expect(
        lines
          ?.map((line) => line.map((token) => token.content).join(''))
          .join('\n')
      ).toBe(source);
      expect(
        lines
          ?.flat()
          .some(
            (token) =>
              token.htmlStyle?.['--shiki-light'] &&
              token.htmlStyle?.['--shiki-dark']
          )
      ).toBe(true);
    }
  );
  it.each([undefined, 'unknown-language'])(
    'falls back for %s',
    async (language) => {
      expect(await highlightCode('original', language)).toBeNull();
    }
  );
  it('falls back for oversized text', async () => {
    expect(
      await highlightCode('x'.repeat(MAX_HIGHLIGHT_LENGTH + 1), 'tsx')
    ).toBeNull();
  });
  it('highlights mixed line endings with normalized lines', async () => {
    const lines = await highlightCode(
      'const a = 1;\r\nconst b = 2;\rconst c = 3;',
      'tsx'
    );
    expect(
      lines?.map((line) => line.map((token) => token.content).join(''))
    ).toEqual(['const a = 1;', 'const b = 2;', 'const c = 3;']);
  });
  it('reuses a completed highlight result', async () => {
    const first = await highlightCode('const cached = 1;', 'typescript');
    expect(first).not.toBeNull();
    expect(await highlightCode('const cached = 1;', 'typescript')).toBe(first);
  });
  it('preserves a trailing blank line for CRLF input', async () => {
    const lines = await highlightCode('const a = 1;\r\n', 'typescript');
    expect(lines?.map((line) => line.map((token) => token.content).join('')).join('\n')).toBe('const a = 1;\n');
  });
});
