import { describe, expect, it } from 'vitest';
import {
  toolCommandHeading,
  toolOutputPreview,
} from '@vibe/ui/lib/tool-output-preview';

describe('operation previews', () => {
  it('keeps an error in the middle of a large result visible with the final result', () => {
    const source = [
      ...Array.from({ length: 40 }, (_, index) => `checking ${index}`),
      'Error: broken invariant',
      'test.ts:42',
      ...Array.from({ length: 40 }, (_, index) => `cleanup ${index}`),
      'exit 1',
    ].join('\n');
    const preview = toolOutputPreview(source, true);
    expect(preview.text).toContain('Error: broken invariant');
    expect(preview.text).toContain('test.ts:42');
    expect(preview.text).toContain('exit 1');
    expect(preview.text.split('\n').length).toBeLessThanOrEqual(6);
    expect(preview.clipped).toBe(true);
    expect(toolOutputPreview('a\nb\nc\nd\n', true)).toEqual({
      text: 'a\nb\nc\nd',
      clipped: false,
    });
  });

  it('clips presentation while retaining source whitespace and valid Unicode', () => {
    const source = '  printf "a\\_b"\t\n  echo end  ';
    expect(toolCommandHeading(source)).toBe('  printf "a\\_b"\t …');
    expect(source).toBe('  printf "a\\_b"\t\n  echo end  ');
    const unicode = 'x'.repeat(239) + '😀end';
    const preview = toolOutputPreview(unicode, false);
    expect(preview.text).not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])/);
    expect(preview.clipped).toBe(true);
    expect(toolOutputPreview('', false).text).toBe('');
  });
});
