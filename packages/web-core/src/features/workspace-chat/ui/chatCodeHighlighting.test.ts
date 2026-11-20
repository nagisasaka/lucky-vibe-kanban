import { describe, expect, it } from 'vitest';
import { highlightCode, MAX_HIGHLIGHT_LENGTH } from '@vibe/ui/lib/shiki-code';

describe('Shiki code highlighting', () => {
  it.each([
    ['tsx', 'export const View = () => <span>hello</span>;'],
    ['rust', 'fn main() { println!("hello"); }'],
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
  it('falls back for oversized and mixed-line-ending text', async () => {
    expect(
      await highlightCode('x'.repeat(MAX_HIGHLIGHT_LENGTH + 1), 'tsx')
    ).toBeNull();
    expect(await highlightCode('a\r\nb', 'tsx')).toBeNull();
  });
});
