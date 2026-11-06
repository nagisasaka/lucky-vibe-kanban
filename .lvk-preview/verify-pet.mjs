// Existing public preview. UI checks block writes; API compatibility probe owns one temporary scratch UUID.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
const base = 'https://preview2.luckyvibekanban.com',
  out = '.build/design-preview/pet-merge';
await mkdir(out, { recursive: true });
const credential = JSON.parse(
  await readFile('.build/design-preview/credentials.json')
);
const browser = await chromium.launch({
  headless: true,
  args: ['--no-sandbox'],
});
const result = { cases: [], errors: [], geometry: [] };
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const login = await context.request.post(base + '/api/access/basic-login', {
    headers: {
      Authorization:
        'Basic ' +
        Buffer.from(credential.username + ':' + credential.password).toString(
          'base64'
        ),
      'x-lvk-csrf': 'basic',
      Origin: base,
    },
    data: {},
  });
  expect(login.ok()).toBe(true);
  const csrf = (await login.json()).csrf;
  const endpoint = base + '/api/scratch/UI_PREFERENCES/' + randomUUID();
  let created = false;
  try {
    const payload = {
      type: 'UI_PREFERENCES',
      data: {
        pet_preferences: { selected: 'lvk-rabbit', hidden: true, pets: [] },
      },
    };
    const saved = await context.request.put(endpoint, {
      headers: { 'x-lvk-csrf': csrf, Origin: base },
      data: { payload },
    });
    expect(saved.ok()).toBe(true);
    created = true;
    const body = await (await context.request.get(endpoint)).json();
    result.backendPetFieldPersisted =
      body.data?.payload?.data?.pet_preferences?.hidden === true;
    result.backendProbe =
      'Dedicated temporary UI_PREFERENCES UUID only; never global preferences';
  } finally {
    if (created) {
      const removed = await context.request.delete(endpoint, {
        headers: { 'x-lvk-csrf': csrf, Origin: base },
        data: { expected_payload: null },
      });
      expect(removed.ok()).toBe(true);
      const after = await (await context.request.get(endpoint)).json();
      expect(after.data).toBeNull();
      result.probeCleaned = true;
    }
  }
  const projects = await (
    await context.request.get(
      base +
        '/api/board/projects?organization_id=00000000-0000-0000-0000-000000000002'
    )
  ).json();
  const project = projects.data.projects[0];
  const page = await context.newPage();
  page.on('pageerror', (e) => result.errors.push(e.message));
  await page.route('**/api/**', (route) =>
    ['GET', 'HEAD'].includes(route.request().method()) ||
    route.request().url().endsWith('/provider-usage/read')
      ? route.fallback()
      : route.fulfill({ json: { success: true, data: null } })
  );
  await page.route('**/api/info', async (route) => {
    const response = await route.fetch(),
      body = await response.json();
    body.data.config.language = 'EN';
    body.data.config.show_release_notes = false;
    await route.fulfill({ response, json: body });
  });
  await page.goto(base + '/projects/' + project.id);
  const panel = page.getByTestId('environment-panel');
  await panel.waitFor({ state: 'attached', timeout: 60000 });
  if (!(await panel.isVisible()))
    await page.getByRole('button', { name: 'Show Right Sidebar' }).click();
  const rabbit = panel.getByRole('button', { name: 'LVK Rabbit', exact: true });
  await expect(rabbit).toBeVisible();
  await expect
    .poll(() =>
      rabbit
        .locator('img')
        .evaluate(
          (img) =>
            img.complete &&
            img.naturalWidth === 1536 &&
            img.naturalHeight === 1872
        )
    )
    .toBe(true);
  await expect(panel.getByTestId('environment-hostname')).not.toHaveText('—');
  for (const label of ['CPU', 'RAM', 'SWAP', 'DISK'])
    await expect(
      panel.getByRole('button', { name: label + ' details', exact: true })
    ).not.toContainText('—');
  await rabbit.click();
  await expect(rabbit).toHaveAttribute('data-pet-state', 'waving');
  await expect(rabbit).toHaveAttribute('data-pet-state', 'idle');
  result.cases.push('Real Rabbit atlas loaded; click waving -> idle');
  for (const viewport of [320, 1440])
    for (const theme of ['light', 'dark']) {
      await page.setViewportSize({ width: viewport, height: 1000 });
      await page.evaluate(
        (theme) =>
          document.documentElement.classList.toggle('dark', theme === 'dark'),
        theme
      );
      await panel.evaluate((el) =>
        Object.assign(el.parentElement.style, {
          width: '300px',
          minWidth: '300px',
          maxWidth: '300px',
          overflow: 'hidden',
        })
      );
      const m = await panel.evaluate((el) => {
        const rect = (e) => {
          const r = e.getBoundingClientRect();
          return { x: r.x, y: r.y, width: r.width, right: r.right };
        };
        return {
          container: rect(el.parentElement),
          panel: rect(el),
          pet: rect(el.querySelector('[data-testid="sidebar-pet"]')),
          cells: [
            ...el
              .querySelector('[data-testid="environment-metrics"]')
              .querySelectorAll('button'),
          ].map((e) => ({
            cell: rect(e),
            parts: [...e.firstElementChild.children].map((s) => ({
              text: s.textContent,
              ...rect(s),
            })),
          })),
        };
      });
      expect(m.container.width).toBe(300);
      for (const c of m.cells) {
        const [label, value] = c.parts;
        expect(label.right).toBeLessThanOrEqual(value.x);
        expect(value.right).toBeLessThanOrEqual(c.cell.right + 0.1);
        expect(value.right).toBeLessThanOrEqual(m.panel.right);
      }
      result.geometry.push({ viewport, theme, ...m });
      await page.screenshot({ path: out + `/live-${viewport}-${theme}.png` });
      const summary = panel
        .getByTestId('sidebar-pet')
        .locator('summary')
        .first();
      await summary.focus();
      await page.keyboard.press('Enter');
      const settings = panel
        .getByTestId('sidebar-pet')
        .locator('details > div')
        .first();
      await expect(settings).toBeVisible();
      const bounds = await settings.boundingBox();
      expect(bounds.x).toBeGreaterThanOrEqual(0);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport);
      const preview = settings.getByLabel('Animation preview');
      await preview.selectOption('running-right');
      await expect(rabbit).toHaveAttribute('data-pet-state', 'running-right');
      await page.screenshot({
        path: out + `/settings-${viewport}-${theme}.png`,
      });
      await preview.selectOption('idle');
      await summary.click();
    }
  result.cases.push(
    '300px real utility container: light/dark at320/1440, all metrics and hostname coexist; keyboard settings and animation preview'
  );
  await expect(
    page.getByRole('button', { name: 'Close utilities' })
  ).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Terminal', exact: true })
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Wiki', exact: true })
  ).toBeVisible();
  expect(result.errors).toEqual([]);
  await writeFile(out + '/live-results.json', JSON.stringify(result, null, 2));
  console.log(
    JSON.stringify(
      {
        cases: result.cases,
        backendPetFieldPersisted: result.backendPetFieldPersisted,
        probeCleaned: result.probeCleaned,
        errors: result.errors,
      },
      null,
      2
    )
  );
} finally {
  await browser.close();
}
