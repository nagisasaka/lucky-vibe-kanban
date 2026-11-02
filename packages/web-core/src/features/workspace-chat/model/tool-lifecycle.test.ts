import i18n from '@/i18n/config';
import { I18nextProvider } from 'react-i18next';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ChatAggregatedToolEntries } from '@vibe/ui/components/ChatAggregatedToolEntries';
import { describe, expect, it } from 'vitest';
import { aggregateToolStatus } from '@vibe/ui/lib/tool-lifecycle';

describe('aggregate tool lifecycle', () => {
  const entry = (status: string, active = false) => ({
    status: { status },
    active,
  });

  it.each(['failed', 'denied', 'timed_out', 'cancelled'])(
    'keeps %s visible among successful and running work',
    (status) => {
      expect(
        aggregateToolStatus([
          entry('success'),
          entry('created', true),
          entry(status),
        ])
      ).toEqual({ status });
    }
  );

  it('requires success evidence from every member', () => {
    expect(aggregateToolStatus([entry('success'), {}])).toEqual({
      status: 'unknown',
    });
    expect(aggregateToolStatus([entry('created')])).toEqual({
      status: 'unknown',
    });
    expect(aggregateToolStatus([])).toEqual({ status: 'unknown' });
    expect(aggregateToolStatus([entry('success'), entry('success')])).toEqual({
      status: 'success',
    });
    expect(
      aggregateToolStatus([entry('created', true), entry('success')])
    ).toEqual({
      status: 'created',
    });
  });
});

it('localizes tool evidence framing while retaining raw paths, output and exit codes', () => {
  for (const language of ['en', 'ja']) {
    const endedAt = '2026-01-02T15:04:05Z';
    const html = renderToStaticMarkup(
      createElement(
        I18nextProvider,
        { i18n: i18n.cloneInstance({ lng: language }) },
        createElement(ChatAggregatedToolEntries, {
          entries: [
            {
              summary: 'Raw operation 原文',
              expansionKey: 'raw-id',
              content: 'Raw output 原文',
              cwd: '/raw/原文',
              status: { status: 'failed' },
              exitCode: 1234,
              durationMs: 1234567,
              endedAt,
              outputState: 'complete',
            },
          ],
          expanded: true,
          isHovered: false,
          onToggle() {},
          onHoverChange() {},
        })
      )
    );
    for (const raw of [
      'Raw operation 原文',
      'Raw output 原文',
      '/raw/原文',
      '1234',
      endedAt,
    ])
      expect(html).toContain(raw);
    expect(html).toContain(new Date(endedAt).toLocaleString(language));
    expect(html).toContain(
      language === 'ja' ? '作業ディレクトリ' : 'Working directory'
    );
    expect(html).toContain('1,234.567');
  }
});
