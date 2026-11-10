import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { expect } from '@playwright/test';

// Real Issue panel on its bg-secondary outer surface; no application data/API.
export async function checkHeadingHierarchy(page) {
  const output = '.build/markdown-heading-review';
  await mkdir(output, { recursive: true });
  const title =
    'Markdown本文の読みやすさと人間の判断待ちを見つけやすくするために、長い日本語のIssueタイトルと本文見出しの階層を整える';
  await page.evaluate((title) => {
    window.changeTitle(title);
    window.changeBody(
      '# 本文の第一見出し\n\n目的と結論を読みやすく伝えます。\n\n## 要件と実現方法\n\n共通の本文スタイルを両レンダラーで保ちます。\n\n### 詳細な確認事項\n\n背景色と保存内容は変更しません。'
    );
    document.querySelector('main').style.display = 'none';
  }, title);
  await expect(page.locator('#issue h3')).toHaveCount(1);
  const measurements = [];
  for (const [name, sizes] of [
    ['before', [1.5, 1.8, 1.45, 1.2]],
    ['moderate', [1.75, 1.5, 1.3, 1.15]],
    ['larger', [1.875, 1.6, 1.4, 1.2]],
    ['final', null],
  ]) {
    const style = sizes
      ? await page.addStyleTag({
          content: `.issue-detail-title {font-size:${sizes[0]}rem} .markdown-body h1 {font-size:${sizes[1]}em} .markdown-body h2 {font-size:${sizes[2]}em} .markdown-body h3 {font-size:${sizes[3]}em}`,
        })
      : null;
    for (const dark of [false, true]) {
      await page.evaluate((dark) => {
        document.documentElement.classList.toggle('dark', dark);
        window.setTheme(dark ? 'dark' : 'light');
      }, dark);
      for (const width of [390, 800]) {
        await page.setViewportSize({ width, height: 1050 });
        await page.locator('#issue').evaluate((e, width) => {
          e.style.width = `${width}px`;
        }, width);
        await page.evaluate(
          () =>
            new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve))
            )
        );
        const input = page.locator('#issue .issue-detail-title');
        await expect
          .poll(() =>
            input.evaluate((e) => e.scrollHeight <= e.clientHeight + 1)
          )
          .toBe(true);
        const geometry = await input.evaluate((e) => ({
          font: parseFloat(getComputedStyle(e).fontSize),
          height: e.clientHeight,
          scrollHeight: e.scrollHeight,
          width: e.clientWidth,
          scrollWidth: e.scrollWidth,
          value: e.value,
        }));
        assert.equal(geometry.value, title);
        assert.ok(geometry.scrollWidth <= geometry.width + 1);
        assert.ok(geometry.scrollHeight <= geometry.height + 1);
        const headings = await page
          .locator('#issue :is(h1,h2,h3)')
          .evaluateAll((es) =>
            es.map((e) => parseFloat(getComputedStyle(e).fontSize))
          );
        if (name === 'final') {
          assert.ok(
            geometry.font > headings[0] &&
              headings[0] > headings[1] &&
              headings[1] > headings[2]
          );
          await input.focus();
          await input.press('End');
          await input.press('!');
          await expect(input).toHaveValue(`${title}!`);
          await input.press('Backspace');
          await expect(input).toHaveValue(title);
          assert.equal(
            await input.evaluate((e) =>
              parseFloat(getComputedStyle(e).fontSize)
            ),
            geometry.font
          );
        }
        if (name === 'final') {
          assert.ok(
            await page
              .locator('#issue')
              .evaluate((e) => e.scrollWidth <= e.clientWidth + 1)
          );
          await page.locator('#issue h1').click();
          await expect(
            page.locator('#issue [contenteditable="true"]')
          ).toHaveCount(1);
          await page.locator('#issue [contenteditable="true"]').click();
          const edited = await page
            .locator('#issue :is(h1,h2,h3)')
            .evaluateAll((es) =>
              es.map((e) => parseFloat(getComputedStyle(e).fontSize))
            );
          assert.deepEqual(edited, headings);
          await page.locator('#issue').screenshot({
            path: `${output}/edit-${width}-${dark ? 'dark' : 'light'}.png`,
          });
          await input.focus();
          await expect(
            page.locator('#issue [contenteditable="true"]')
          ).toHaveCount(0);
        }
        measurements.push({ name, dark, width, title: geometry, headings });
        await page.locator('#issue').screenshot({
          path: `${output}/${name}-${width}-${dark ? 'dark' : 'light'}.png`,
        });
      }
    }
    await style?.evaluate((e) => e.remove());
  }
  // Other long-form consumers share the same heading scale; editor/view match.
  await page.evaluate(() => {
    document.querySelector('main').style.display = '';
  });
  for (const editing of [false, true]) {
    await page.evaluate((editing) => window.setEditing(editing), editing);
    await expect(page.locator('#lexical h3')).toHaveCount(1);
    const sizes = [];
    for (const id of ['lexical', 'preview']) {
      sizes.push(
        await page
          .locator(`#${id} :is(h1,h2,h3)`)
          .evaluateAll((es) => es.map((e) => getComputedStyle(e).fontSize))
      );
    }
    assert.deepEqual(sizes[0], sizes[1]);
    measurements.push({ editing, rendererHeadingSizes: sizes });
  }
  await writeFile(
    `${output}/measurements.json`,
    JSON.stringify(measurements, null, 2)
  );
  await page.evaluate(() => {
    window.setEditing(false);
    window.changeTitle('本文と目次');
    window.changeBody(window.source);
    document.querySelector('#issue').style.width = '390px';
    document.documentElement.classList.remove('dark');
    window.setTheme('light');
    window.scrollTo(0, 0);
  });
  await expect(page.locator('#lexical h1')).toHaveText(
    'Markdown本文の読みやすさ'
  );
}
