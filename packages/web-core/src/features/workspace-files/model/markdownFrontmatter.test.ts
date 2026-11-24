import { describe, expect, it } from 'vitest';
import { splitMarkdownFrontmatter } from './markdownFrontmatter';

describe('Markdown file frontmatter', () => {
  it.each(['\n', '\r\n'])(
    'preserves body and raw YAML with %j newlines',
    (nl) => {
      const metadata = [
        'title: Example',
        'tags: [one, two]',
        'nested:',
        '  key: value',
        'description: |',
        '  line one',
        '  line two',
        '',
      ].join(nl);
      const body = `${nl}# Body${nl}${nl}---${nl}Text`;
      expect(
        splitMarkdownFrontmatter(`\uFEFF---${nl}${metadata}---${nl}${body}`)
      ).toEqual({ body, metadata });
    }
  );
  it('accepts a YAML document end and a closing fence at EOF', () => {
    expect(
      splitMarkdownFrontmatter('---\ntitle: Example\n...\n# Body').body
    ).toBe('# Body');
    expect(splitMarkdownFrontmatter('---\ntitle: Example\n---').body).toBe('');
  });
  it.each([
    '# Body\n---\nText',
    '\n---\ntitle: Example\n---\n# Body',
    '---\ntitle: Example\n# Body',
    '---\ntitle: [broken\n---\n# Body',
    '---\ntitle: one\ntitle: two\n---\n# Body',
    '---\nOrdinary paragraph\n---\n# Body',
    '---\n---\n# Body',
    '---\n# Heading\nParagraph\n---\nTail',
  ])('keeps ambiguous or malformed input intact: %s', (content) => {
    expect(splitMarkdownFrontmatter(content)).toEqual({
      body: content,
      metadata: null,
    });
  });
  it('keeps aliases as source without expanding them', () => {
    const metadata = 'a: &a [one]\nb: [*a, *a]\n';
    expect(splitMarkdownFrontmatter(`---\n${metadata}---\nBody`).metadata).toBe(
      metadata
    );
  });
});
