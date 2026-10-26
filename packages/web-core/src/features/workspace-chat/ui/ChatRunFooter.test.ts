import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, it, expect } from 'vitest';
import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import { ChatRunFooter } from '@vibe/ui/components/ChatRunFooter';
import { ChatCollapsedThinking } from '@vibe/ui/components/ChatCollapsedThinking';
import { ChatThinkingMessage } from '@vibe/ui/components/ChatThinkingMessage';
void i18next.use(initReactI18next).init({
  lng: 'en',
  initImmediate: false,
  interpolation: { escapeValue: false },
  resources: {
    en: { tasks: { execution: { completedAt: 'Completed: {{time}}' } } },
  },
});
const render = (props: Parameters<typeof ChatRunFooter>[0]) =>
  renderToStaticMarkup(createElement(ChatRunFooter, props));
describe('observed run completion footer', () => {
  it('keeps the real completion date alongside duration', () => {
    const markup = render({
      startedAt: '2026-10-25T10:00:00Z',
      endedAt: '2026-10-25T10:00:13Z',
      active: false,
    });
    expect(markup).toContain('13s');
    expect(markup).toContain('dateTime="2026-10-25T10:00:13Z"');
    expect(markup).toContain('Completed:');
    expect(markup).not.toContain('&#x2F;');
  });
  it('never fills missing or invalid completion evidence', () => {
    for (const endedAt of [null, 'invalid'])
      expect(
        render({ startedAt: '2026-10-25T10:00:00Z', endedAt, active: false })
      ).not.toContain('<time');
    expect(
      render({ startedAt: 'invalid', endedAt: null, active: false })
    ).not.toContain('0s');
  });
});
describe('provider notes initially readable', () => {
  const common = {
    renderMarkdown: ({ content }: { content: string }) =>
      createElement('div', null, content),
  };
  it('hides notes that contain only whitespace', () => {
    expect(
      renderToStaticMarkup(
        createElement(ChatThinkingMessage, {
          ...common,
          content: '  \n ',
          active: false,
          label: 'Plan',
        })
      )
    ).toBe('');
    expect(
      renderToStaticMarkup(
        createElement(ChatCollapsedThinking, {
          ...common,
          entries: [{ content: ' ', expansionKey: 'empty', label: 'Plan' }],
          expanded: false,
          isHovered: false,
          onToggle: () => {},
          onHoverChange: () => {},
        })
      )
    ).toBe('');
  });
  it('keeps original first line visible without expanding a completed group', () => {
    const markup = renderToStaticMarkup(
      createElement(ChatCollapsedThinking, {
        ...common,
        entries: [
          {
            content: 'Inspect README.md\nThen run tests',
            expansionKey: 'note',
            label: 'Plan',
          },
        ],
        expanded: false,
        isHovered: false,
        onToggle: () => {},
        onHoverChange: () => {},
      })
    );
    expect(markup).toContain('Inspect README.md');
    expect(markup).not.toContain('providerNotes');
    expect(markup).not.toContain('Then run tests');
  });
});

it('shows the received summary line as its heading and only animates active text', () => {
  const props = {
    content: 'Inspecting the workspace\nNext detail',
    label: 'Reasoning summary',
    renderMarkdown: ({ content }: { content: string }) =>
      createElement('div', null, content),
  };
  const completed = renderToStaticMarkup(
    createElement(ChatThinkingMessage, { ...props, active: false })
  );
  expect(completed).toContain('Inspecting the workspace');
  expect(completed).not.toContain('vk-activity-text');
  const active = renderToStaticMarkup(
    createElement(ChatThinkingMessage, { ...props, active: true })
  );
  expect(active).toContain('vk-activity-text');
});
