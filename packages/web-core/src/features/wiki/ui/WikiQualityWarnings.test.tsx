import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { WikiQualityWarnings } from './WikiQualityWarnings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('Wiki publication quality warnings', () => {
  it('keeps an unqualified publication free of warning labels', () => {
    expect(renderToStaticMarkup(<WikiQualityWarnings warnings={[]} />)).toBe(
      ''
    );
  });
  it('shows residual findings and their evidence without treating publication as a pass', () => {
    const html = renderToStaticMarkup(
      <WikiQualityWarnings
        warnings={[
          {
            node_id: 're-review',
            summary: 'Coverage gap remains',
            findings: [
              {
                title: 'Missing cancellation contract',
                description: '<unsafe markup>',
                evidence_paths: ['openwiki/runtime.md', 'src/runtime.rs'],
              },
            ],
          },
        ]}
      />
    );
    expect(html).toContain('workspaceWiki.qualityWarnings');
    expect(html).toContain('Coverage gap remains');
    expect(html).toContain('openwiki/runtime.md');
    expect(html).toContain('&lt;unsafe markup&gt;');
  });
  it('exposes a missing review even without page findings', () => {
    const html = renderToStaticMarkup(
      <WikiQualityWarnings
        warnings={[
          { node_id: 'review', summary: 'レビュー結果なし', findings: [] },
        ]}
      />
    );
    expect(html).toContain('レビュー結果なし');
  });
});
