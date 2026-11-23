import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { RunSpeechToggle } from './RunSpeechToggle';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (_key: string, fallback: string) => fallback }),
}));

describe('RunSpeechToggle', () => {
  it('opens with speech off and does not subscribe until enabled', () => {
    const request = vi.spyOn(globalThis, 'fetch');
    vi.stubGlobal('SpeechSynthesisUtterance', class {});
    vi.stubGlobal('window', {
      speechSynthesis: { cancel: vi.fn(), speak: vi.fn() },
    });
    try {
      const html = renderToStaticMarkup(
        createElement(RunSpeechToggle, { projectId: 'project' })
      );
      expect(html).toContain('aria-pressed="false"');
      expect(html).toContain('Turn on Run speech');
      expect(request).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
      request.mockRestore();
    }
  });
});
