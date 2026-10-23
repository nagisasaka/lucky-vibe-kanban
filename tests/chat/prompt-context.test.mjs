import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';

test('prompt context preserves user text, full copy and guidance toggle semantics', async () => {
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const bundle = await build({
    entryPoints: [root + 'tests/chat/prompt-context.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    alias: { '@': root + 'packages/web-core/src', shared: root + 'shared' },
    nodePaths: [root + 'packages/web-core/node_modules'],
  });
  const require = createRequire(root + 'packages/web-core/package.json');
  const styles = await require('postcss')([
    require('tailwindcss')({
      ...require('tailwindcss/loadConfig')(
        root + 'packages/local-web/tailwind.new.config.js'
      ),
      safelist: ['dark', 'bg-primary', 'text-normal'],
      content: [
        root +
          'packages/ui/src/components/{ChatUserMessage,ChatEntryContainer,PromptContext}.tsx',
        root +
          'packages/web-core/src/features/pipeline/ui/CardContextSection.tsx',
      ],
    }),
  ]).process(
    readFileSync(
      root + 'packages/web-core/src/app/styles/new/index.css',
      'utf8'
    ).replace(/^@(import|config).*$/gm, ''),
    { from: undefined }
  );
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox'],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.setContent(
      '<body class="bg-primary text-normal"><div id="root"></div></body>'
    );
    await page.addStyleTag({ content: styles.css });
    await page.evaluate(() =>
      Object.defineProperty(navigator, 'clipboard', {
        value: {
          writeText: async (text) => {
            window.copied = text;
          },
        },
        configurable: true,
      })
    );
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await page.waitForFunction(() => !!window.setContent);
    const start = '<!-- lvk:card-context:start -->';
    const end = '<!-- lvk:card-context:end -->';
    const block = `${start}\n補助案内\n${end}`;
    const visible = [
      '通常の依頼。Shared directoriesとRepository Memoryを調べる',
      `前文 ${start}\nユーザー本文\n${end}`,
      `${start}\n不完全なユーザー本文`,
      `前文\n${end}\n後文`,
      `\`\`\`md\n${block}\n\`\`\``,
      `~~~md\n${block}\n~~~`,
      `> ${start}\n> 引用\n> ${end}`,
      `    ${start}\n    インデントされたコード\n    ${end}`,
      `${start}\n${start}\n曖昧\n${end}`,
      `${start}\n誤った境界\n<!-- vk:pipeline:end -->`,
    ];
    for (const content of visible) {
      const parts = await page.evaluate((text) => window.split(text), content);
      assert.deepEqual(parts, [{ content, auxiliary: false }]);
      await page.evaluate((text) => window.setContent(text), content);
      await expect(page.locator('#message details')).toHaveCount(0);
      await expect(page.locator('#message pre')).toHaveText(content);
    }
    for (const content of [
      `前\n${block}\n後`,
      `前\r\n${block.replaceAll('\n', '\r\n')}\r\n後`,
      `${block}\n中間のユーザー入力\n${block}`,
      '<!-- vk:pipeline:start -->\n旧案内\n<!-- vk:pipeline:end -->',
      '<!-- lvk:shared-directories:start -->\n共有案内\n<!-- lvk:shared-directories:end -->',
    ]) {
      const parts = await page.evaluate((text) => window.split(text), content);
      assert.equal(parts.map((p) => p.content).join(''), content);
      assert.ok(parts.some((p) => p.auxiliary));
    }
    await page.evaluate(() => window.setContent(window.initial));
    const message = page.locator('#message');
    await expect(message.locator('details')).toHaveCount(2);
    await expect(message.locator('pre').first()).toBeVisible();
    await expect(message.locator('pre').first()).toContainText(
      'ユーザーの元入力'
    );
    await expect(
      message.locator('details').first().locator('pre')
    ).toBeHidden();
    await message.getByText('補助情報', { exact: true }).click();
    await expect(
      message.locator('details').first().locator('pre')
    ).toBeVisible();
    await message.getByText('元の全文を表示', { exact: true }).focus();
    await page.keyboard.press('Enter');
    assert.equal(
      await message.locator('details').last().locator('pre').textContent(),
      await page.evaluate(() => window.initial)
    );
    await message.getByRole('button', { name: '元の全文をコピー' }).click();
    assert.equal(
      await page.evaluate(() => window.copied),
      await page.evaluate(() => window.initial)
    );
    await message
      .getByRole('button', { name: 'メッセージを編集', exact: true })
      .click();
    assert.equal(
      await page.evaluate(() => window.edited),
      await page.evaluate(() => window.initial)
    );
    await page.evaluate(() => {
      navigator.clipboard.writeText = async () => {
        throw Error('denied');
      };
    });
    await message.getByRole('button', { name: '元の全文をコピー' }).click();
    await expect(message.getByRole('status')).toContainText(
      'コピーできませんでした'
    );
    const settings = page.locator('#settings');
    await settings.locator('summary').click();
    const toggle = settings.getByRole('checkbox', {
      name: '共有ディレクトリの補助案内',
    });
    await expect(toggle).toBeChecked();
    await expect(settings).toContainText(
      'OFFでも共有領域とRepository Memoryは無効になりません'
    );
    await toggle.uncheck();
    assert.ok(
      !(await page.evaluate(() => window.description)).includes(
        'lvk:shared-directories:start'
      )
    );
    assert.ok(
      (await page.evaluate(() => window.description)).startsWith(
        'ユーザーの元入力'
      )
    );
    await toggle.check();
    assert.equal(
      await page.evaluate(() => window.description),
      await page.evaluate(() => window.initial)
    );
    for (const width of [390, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      for (const dark of [false, true]) {
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle('dark', dark),
          dark
        );
        await expect(toggle).toBeVisible();
        await expect(
          message.getByRole('button', { name: '元の全文をコピー' })
        ).toBeVisible();
        assert.ok(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth
          )
        );
        if (process.env.PROMPT_CONTEXT_SCREENSHOT_DIR) {
          await page.screenshot({
            path: `${process.env.PROMPT_CONTEXT_SCREENSHOT_DIR}/${width}-${dark ? 'dark' : 'light'}.png`,
            fullPage: true,
          });
        }
      }
    }
    await message.getByText('補助情報', { exact: true }).click();
    await message.getByText('元の全文を表示', { exact: true }).click();
    await expect(message.locator('details pre').first()).toBeHidden();
    if (process.env.PROMPT_CONTEXT_SCREENSHOT_DIR)
      await page.screenshot({
        path: `${process.env.PROMPT_CONTEXT_SCREENSHOT_DIR}/collapsed-dark.png`,
        fullPage: true,
      });
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
