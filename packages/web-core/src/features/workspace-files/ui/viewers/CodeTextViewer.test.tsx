import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { CodeTextViewer } from './CodeTextViewer';

describe('read-only code viewer rendering', () => {
  it('escapes HTML-like file content and keeps the existing row/selection markup', () => {
    const html = renderToStaticMarkup(
      createElement(CodeTextViewer, {
        content:
          '/* first\nsecond */\nconst html = "<img src=x onerror=alert(1)>";\n',
        language: 'typescript',
        line: 3,
      })
    );
    expect(html).toContain('--syntax-comment');
    expect(html).toContain('--syntax-keyword');
    expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;');
    expect(html).not.toContain('<img');
    expect(html).not.toContain('contenteditable');
    expect(html).not.toContain('textarea');
    expect(html.match(/<code /g)).toHaveLength(4);
    expect(html.match(/aria-current="location"/g)).toHaveLength(1);
    expect(html).toContain('whitespace-pre');
  });
  it('leaves unknown-language text unstyled and out-of-range jumps unselected', () => {
    const html = renderToStaticMarkup(
      createElement(CodeTextViewer, {
        content: '  plain\ttext\n',
        language: 'unknown-language',
        line: 99,
      })
    );
    expect(html).not.toContain('--syntax-');
    expect(html).not.toContain('aria-current');
    expect(html).toContain('  plain\ttext');
    expect(html.match(/<code /g)).toHaveLength(2);
  });
});
