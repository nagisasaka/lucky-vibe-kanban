// Component-local regression of capture handling; no application/API coverage.
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import path from 'node:path';

const root = process.cwd();
const bundle = await build({
  stdin: {
    contents: `
      import React, {useState} from 'react';
      import {createRoot} from 'react-dom/client';
      import {useTouchClickGuard} from ${JSON.stringify(path.join(root, 'packages/web-core/src/features/kanban/ui/useTouchClickGuard.ts'))};
      function Fixture() {
        const guard = useTouchClickGuard();
        const [count,setCount] = useState(0);
        return <><div {...guard} style={{padding:40,width:200}}>
          <button onClick={()=>setCount(n=>n+1)}>Tap</button>
        </div><output>{count}</output></>;
      }
      createRoot(document.getElementById('root')).render(<Fixture/>);
    `,
    loader: 'tsx',
    resolveDir: path.join(root, 'packages/web-core'),
  },
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
});
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
  });
  await page.setContent(
    '<meta name="viewport" content="width=device-width,initial-scale=1"><div id="root"></div>'
  );
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  const button = page.getByRole('button', { name: 'Tap' });
  const b = await button.boundingBox();
  const start = { x: b.x + b.width / 2, y: b.y + b.height / 2, id: 1 };
  const cdp = await page.context().newCDPSession(page);
  const send = (type, points = []) =>
    cdp.send('Input.dispatchTouchEvent', { type, touchPoints: points });
  async function moveAndReturn(dx) {
    await send('touchStart', [start]);
    await send('touchMove', [{ ...start, x: start.x + dx }]);
    await send('touchMove', [start]);
    await send('touchEnd');
    await page.waitForTimeout(100);
  }
  await moveAndReturn(4);
  await expect(page.locator('output')).toHaveText('1');
  await moveAndReturn(7);
  await expect(page.locator('output')).toHaveText('1');
  // A canceled gesture must not poison subsequent keyboard or mouse input.
  await send('touchStart', [start]);
  await send('touchCancel');
  await button.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('output')).toHaveText('2');
  await page.mouse.click(start.x, start.y);
  await expect(page.locator('output')).toHaveText('3');
  await page.touchscreen.tap(start.x, start.y);
  await expect(page.locator('output')).toHaveText('4');
  await send('touchStart', [start]);
  await send('touchStart', [start, { ...start, id: 2, x: start.x + 10 }]);
  await send('touchEnd');
  await page.waitForTimeout(100);
  await expect(page.locator('output')).toHaveText('4');
  console.log(
    'PASS: 4px tap, 7px out-and-back rejection, cancel, keyboard/mouse recovery, next tap, multi-touch; nested control capture'
  );
} finally {
  await browser.close();
}
