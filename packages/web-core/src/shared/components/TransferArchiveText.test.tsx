import { expect, it, vi } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { TransferArchiveText } from './TransferArchiveText';
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
it('keeps archived text intact without fetching or activating images, links or old paths', () => {
  const body =
    '[file](attachment://old) ![secret](http://old-root/image) <img src="/old-root/secret"> [local](file:///old-root)';
  const html = renderToStaticMarkup(<TransferArchiveText content={body} />);
  expect(html).not.toMatch(/<(?:a|img|iframe|script)\b/);
  expect(html).toContain('attachment://old');
  expect(html).toContain('file:///old-root');
  expect(html).toContain('&lt;img');
  expect(body).toContain('<img src="/old-root/secret">');
});
