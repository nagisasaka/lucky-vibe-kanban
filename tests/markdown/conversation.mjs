import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { expect } from '@playwright/test';

export async function checkConversation(page) {
  const output = '.build/markdown-conversation-review';
  await mkdir(output, { recursive: true });
  const summary = page.locator('#thinking summary');
  await expect(summary.locator('strong')).toHaveText(
    'Checking sandbox behavior'
  );
  await expect(summary).not.toContainText('**');
  await summary.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#thinking details')).toHaveAttribute('open', '');
  await expect(
    page.locator('#thinking details .markdown-body strong')
  ).toHaveText(['Checking sandbox behavior', '太字']);
  await page.keyboard.press('Enter');
  await expect(page.locator('#thinking details')).not.toHaveAttribute('open');
  const group = page.locator('#thinking-group > div > button');
  await expect(group.locator('strong')).toHaveText('Checking sandbox behavior');
  const row = page.locator('#thinking-group p.truncate');
  await expect(row.locator('em')).toHaveText('Next step');
  await expect(row.locator('code')).toHaveText('code');
  await expect(row.locator('del')).toHaveText('old');
  assert.equal(await page.locator('#thinking :is(a,img,script)').count(), 0);
  await group.focus();
  await page.keyboard.press('Space');
  await expect(group).toHaveAttribute('aria-expanded', 'true');
  await expect(page.locator('#thinking-group .markdown-body')).toHaveCount(2);
  await group.click();
  await expect(group).toHaveAttribute('aria-expanded', 'false');
  await expect(page.locator('#thinking-group .markdown-body')).toHaveCount(0);
  assert.equal(
    await row.evaluate((e) => getComputedStyle(e).textOverflow),
    'ellipsis'
  );
  assert.ok(await row.evaluate((e) => e.scrollWidth > e.clientWidth));

  // The same canonical source on chat, Issue and Wiki/file renderer paths.
  await page.evaluate(() => window.changeBody(window.referenceMarkdown));
  for (const id of ['conversation', 'issue', 'preview']) {
    const body = page.locator(`#${id} .markdown-body`);
    await expect(body.locator('[data-markdown-alert="decision"]')).toHaveCount(
      1
    );
    const label = body.locator('blockquote').filter({ hasText: '人間の意図' });
    assert.equal(await label.getAttribute('data-markdown-alert'), null);
    await expect(label.locator('strong, b')).toHaveText('人間の意図');
    assert.ok(
      await label
        .locator('strong, b')
        .evaluate((e) => Number(getComputedStyle(e).fontWeight) >= 600)
    );
    // A bold marker in a normal paragraph remains ordinary text, not an alert.
    await expect(
      body.locator('p').filter({ hasText: '[!DECISION] Claude' })
    ).toHaveCount(1);
  }
  assert.deepEqual(await page.evaluate(() => window.changes), []);
  await page.setViewportSize({ width: 780, height: 900 });
  await page
    .locator('#conversation')
    .screenshot({ path: `${output}/quotes-light.png` });
  for (const dark of [false, true]) {
    await page.evaluate(
      (dark) => document.documentElement.classList.toggle('dark', dark),
      dark
    );
    await page.locator('#thinking').screenshot({
      path: `${output}/thinking-${dark ? 'dark' : 'light'}.png`,
    });
  }
  await page.evaluate(() => {
    window.changeBody(window.source);
    document.documentElement.classList.remove('dark');
    window.scrollTo(0, 0);
  });
  await expect(page.locator('#lexical h1')).toHaveText(
    'Markdown本文の読みやすさ'
  );
}
