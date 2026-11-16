import { expect, it, vi } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ProjectIssueEditor } from './ProjectIssueEditor';
import { useImportedProject } from '@/shared/hooks/useImportedProject';
vi.mock('@/shared/hooks/useImportedProject', () => ({
  useImportedProject: vi.fn(),
}));
vi.mock('./WYSIWYGEditor', () => ({
  default: () => <div data-rich-editor="enabled" />,
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
it('uses literal read and edit surfaces for imported or unverified project text', () => {
  vi.mocked(useImportedProject).mockReturnValue({
    keepTextInert: true,
  } as ReturnType<typeof useImportedProject>);
  const body = '<img src="/old/secret"> ![file](attachment://old)';
  const read = renderToStaticMarkup(
    <ProjectIssueEditor projectId="imported" value={body} disabled />
  );
  const edit = renderToStaticMarkup(
    <ProjectIssueEditor projectId="imported" value={body} />
  );
  expect(read).not.toContain('data-rich-editor');
  expect(read).not.toMatch(/<(?:a|img|iframe)\b/);
  expect(read).toContain('attachment://old');
  expect(edit).toContain('<textarea');
  expect(edit).toContain('&lt;img');
  expect(edit).not.toContain('data-rich-editor');
});
it('preserves existing rich editing for a confirmed ordinary Project', () => {
  vi.mocked(useImportedProject).mockReturnValue({
    keepTextInert: false,
  } as ReturnType<typeof useImportedProject>);
  expect(
    renderToStaticMarkup(
      <ProjectIssueEditor projectId="ordinary" value="body" disabled />
    )
  ).toContain('data-rich-editor="enabled"');
});
