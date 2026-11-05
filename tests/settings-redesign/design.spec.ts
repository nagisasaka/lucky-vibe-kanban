import { test, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
const directory = '.build/design-preview/settings-redesign';
for (const width of [360, 768, 1440])
  for (const language of ['en', 'ja'])
    for (const theme of ['light', 'dark']) {
      test(`settings design ${width} ${language} ${theme}: layout and save/discard`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height: 1000 });
        const errors: string[] = [];
        page.on('pageerror', (e) => errors.push(e.message));
        await page.goto(
          `/?section=notifications&lang=${language}&theme=${theme}`
        );
        await page
          .getByRole('button', { name: '設定を開く', exact: true })
          .click();
        const dialog = page.getByRole('dialog', {
          name: language === 'ja' ? '設定' : 'Settings',
          exact: true,
        });
        await expect(dialog).toBeVisible();
        await page.evaluate(async () => {
          await document.fonts.ready;
          await Promise.all(
            document
              .getAnimations()
              .filter(
                (a) => a.effect?.getComputedTiming().iterations !== Infinity
              )
              .map((a) => a.finished.catch(() => undefined))
          );
        });
        await expect(dialog).toHaveCSS('opacity', '1');
        const geometry = await dialog.evaluate((el) => ({
          dialog: el.getBoundingClientRect().toJSON(),
          viewport: innerWidth,
          scroll: document.documentElement.scrollWidth,
        }));
        expect(geometry.dialog.width).toBeLessThanOrEqual(width);
        expect(geometry.scroll).toBeLessThanOrEqual(width);
        if (width >= 768) {
          const nav = dialog.getByRole('navigation');
          await expect(nav.locator('[aria-current="page"]')).toHaveCount(1);
          await expect(nav.getByRole('button').last()).toBeInViewport();
        }
        await mkdir(directory, { recursive: true });
        await dialog.screenshot({
          path: `${directory}/${width}-${language}-${theme}.png`,
          animations: 'disabled',
        });
        const sound = page.locator('#sound-enabled');
        await sound.check();
        const save = page.getByRole('button', {
          name: language === 'ja' ? '保存' : 'Save',
          exact: true,
        });
        await expect(save).toBeInViewport();
        const saveBox = await save.boundingBox();
        expect(saveBox!.x).toBeGreaterThanOrEqual(0);
        expect(saveBox!.x + saveBox!.width).toBeLessThanOrEqual(width);
        await save.click();
        await expect(sound).toBeChecked();
        await sound.uncheck();
        await page
          .getByRole('button', {
            name: language === 'ja' ? '破棄' : 'Discard',
            exact: true,
          })
          .click();
        await expect(sound).toBeChecked();
        expect(errors).toEqual([]);
        await writeFile(
          `${directory}/${width}-${language}-${theme}.json`,
          JSON.stringify({ geometry, saveBox, errors }, null, 2)
        );
      });
    }
