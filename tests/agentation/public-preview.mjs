// Generated page uses the existing public Vite server, without accessing authenticated app data.
import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
const base = 'https://preview2.luckyvibekanban.com';
const output = '.build/design-preview/agentation';
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const result = {
  authenticatedAppVerified: false,
  errors: [],
  externalRequests: [],
  consoleErrors: [],
  responses: [],
};
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  page.on('console', (message) => {
    if (message.type() === 'error') result.consoleErrors.push(message.text());
  });
  page.on('response', (response) =>
    result.responses.push({
      path: new URL(response.url()).pathname,
      status: response.status(),
    })
  );
  page.on('pageerror', (error) => result.errors.push(error.message));
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.origin !== base && !['data:', 'blob:'].includes(url.protocol))
      result.externalRequests.push(url.origin + url.pathname);
  });
  const wrapperSource = await (
    await fetch(base + '/src/app/entry/DevAgentation.tsx')
  ).text();
  const bootstrapSource = await (
    await fetch(base + '/src/app/entry/Bootstrap.tsx')
  ).text();
  const reactPath = wrapperSource.match(/"([^"]+\/react\.js[^"]*)"/)[1];
  const domPath = bootstrapSource.match(
    /"([^"]+\/react-dom_client\.js[^"]*)"/
  )[1];
  await page.route(base + '/__lvk_agentation_fixture', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<!doctype html><title>Isolated public Vite Agentation</title><div id="root"></div>
  <script type="module">
    import RefreshRuntime from '/@react-refresh';
    RefreshRuntime.injectIntoGlobalHook(window);
    window.$RefreshReg$ = () => {};
    window.$RefreshSig$ = () => type => type;
    window.__vite_plugin_react_preamble_installed__ = true;
  </script>
  <script type="module">
    import '/@vite/client';
    import React from '${reactPath}';
    import ReactDOM from '${domPath}';
    import {DevAgentation} from '/src/app/entry/DevAgentation.tsx';
    ReactDOM.createRoot(document.getElementById('root')).render(React.createElement(React.Fragment,null,React.createElement('h1',null,'Generated preview fixture'),React.createElement(DevAgentation)));
  </script>`,
    })
  );
  await page.goto(base + '/__lvk_agentation_fixture');
  await expect(
    page.getByRole('button', { name: 'Start feedback mode' })
  ).toBeVisible({ timeout: 30000 });
  await page.screenshot({ path: output + '/public-vite-fixture.png' });
  expect(result.errors).toEqual([]);
  expect(result.externalRequests).toEqual([]);
  result.status = 'passed';
  result.observation =
    'Existing public Vite serves lazy Agentation dependency and wrapper to generated fixture; no authenticated app data accessed';
} catch (error) {
  result.status = 'failed';
  result.error = error.message;
  throw error;
} finally {
  await browser.close();
  await writeFile(
    output + '/public-vite-results.json',
    JSON.stringify(result, null, 2)
  );
  console.log(JSON.stringify(result, null, 2));
}
