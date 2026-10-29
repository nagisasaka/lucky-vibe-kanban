import { describe, expect, it, vi } from 'vitest';
import { PrismTokenizer } from '@lexical/code';
import { MAX_CHAT_HIGHLIGHT_LENGTH } from '@vibe/ui/lib/chat-code-highlighting';
import {
  codeHighlightLines,
  MAX_FILE_HIGHLIGHT_LINE_LENGTH,
} from './codeHighlightLines';

const text = (lines: ReturnType<typeof codeHighlightLines>) =>
  lines.map((line) =>
    typeof line === 'string'
      ? line
      : line.map((segment) => segment.text).join('')
  );
const types = (lines: ReturnType<typeof codeHighlightLines>) =>
  lines.flatMap((line) =>
    typeof line === 'string' ? [] : line.flatMap((segment) => segment.types)
  );

describe('read-only file syntax lines', () => {
  it.each([
    ['typescript', 'const answer: number = 42;'],
    ['javascript', 'const answer = true;'],
    ['rust', 'fn main() { let answer = 42; }'],
    ['json', '{"answer": 42, "ok": true}'],
  ])('highlights %s without changing text', (language, source) => {
    const lines = codeHighlightLines(source, language);
    expect(text(lines)).toEqual([source]);
    expect(types(lines).length).toBeGreaterThan(0);
  });

  it('retains multiline comments and mixed line endings', () => {
    const lines = codeHighlightLines(
      '/* first\r\nsecond\rthird */\nconst x = 1;\n',
      'typescript'
    );
    expect(text(lines)).toEqual([
      '/* first',
      'second',
      'third */',
      'const x = 1;',
      '',
    ]);
    expect(lines[1]).toEqual([{ text: 'second', types: ['comment'] }]);
    expect(types([lines[2]])).toContain('comment');
    expect(types([lines[3]])).toContain('keyword');
  });

  it('preserves multiline templates, nested interpolation and raw Rust strings', () => {
    const ts = codeHighlightLines(
      'const s = `first\n${1 + 2}\nlast`;',
      'typescript'
    );
    expect(text(ts)).toEqual(['const s = `first', '${1 + 2}', 'last`;']);
    expect(types([ts[1]])).toContain('interpolation');
    expect(types([ts[2]])).toContain('string');
    const rust = codeHighlightLines('let s = r#"first\nsecond"#;', 'rust');
    expect(text(rust)).toEqual(['let s = r#"first', 'second"#;']);
    expect(types([rust[1]])).toContain('string');
  });

  it.each([
    undefined,
    null,
    'unknown-language',
    'text',
    'constructor',
    '__proto__',
  ])('falls back for %s', (language) => {
    const source = '<script>alert("safe")</script>\n\t日本語  \n';
    const lines = codeHighlightLines(source, language);
    expect(text(lines)).toEqual(source.split('\n'));
    expect(types(lines)).toEqual([]);
  });

  it('keeps empty, oversized and minified text plain and complete', () => {
    for (const source of [
      '',
      'const x = \0;',
      'a'.repeat(MAX_FILE_HIGHLIGHT_LINE_LENGTH + 1),
      'x;\n'.repeat(MAX_CHAT_HIGHLIGHT_LENGTH),
    ]) {
      const lines = codeHighlightLines(source, 'typescript');
      expect(text(lines)).toEqual(source.split('\n'));
      expect(types(lines)).toEqual([]);
    }
  });

  it('does not tokenize oversized or minified input', () => {
    const tokenizer = vi.spyOn(PrismTokenizer, 'tokenize');
    try {
      for (const source of [
        'const x = "' + 'x'.repeat(MAX_FILE_HIGHLIGHT_LINE_LENGTH) + '";',
        'const x = 1;\n'.repeat(MAX_CHAT_HIGHLIGHT_LENGTH),
      ]) {
        const lines = codeHighlightLines(source, 'typescript');
        expect(types(lines)).toEqual([]);
        expect(text(lines)).toEqual(source.split('\n'));
      }
      expect(tokenizer).not.toHaveBeenCalled();
    } finally {
      tokenizer.mockRestore();
    }
  });

  it('falls back when tokenization throws or produces too many tokens', () => {
    const tokenizer = vi
      .spyOn(PrismTokenizer, 'tokenize')
      .mockImplementationOnce(() => {
        throw new Error('grammar failure');
      });
    try {
      expect(codeHighlightLines('const x = 1;', 'typescript')).toEqual([
        'const x = 1;',
      ]);
    } finally {
      tokenizer.mockRestore();
    }
    const source = 'x;\n'.repeat(3000);
    const lines = codeHighlightLines(source, 'javascript');
    expect(text(lines)).toEqual(source.split('\n'));
    expect(types(lines)).toEqual([]);
    const multiline = '/*' + 'a\n'.repeat(5000) + '*/';
    const fragments = codeHighlightLines(multiline, 'javascript');
    expect(text(fragments)).toEqual(multiline.split('\n'));
    expect(types(fragments)).toEqual([]);
  });
});
