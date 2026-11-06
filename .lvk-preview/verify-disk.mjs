import { chromium, expect } from '@playwright/test';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
const base = 'https://preview2.luckyvibekanban.com',
  out = '.build/design-preview/disk';
await mkdir(out, { recursive: true });
const credentials = JSON.parse(
  await readFile('.build/design-preview/credentials.json')
);
const browser = await chromium.launch({
  headless: true,
  args: ['--no-sandbox'],
});
const result = { measurements: [], errors: [] };
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const login = await context.request.post(base + '/api/access/basic-login', {
    headers: {
      Authorization:
        'Basic ' +
        Buffer.from(credentials.username + ':' + credentials.password).toString(
          'base64'
        ),
      'x-lvk-csrf': 'basic',
      Origin: base,
    },
    data: {},
  });
  if (!login.ok()) throw Error('Login failed');
  const projects = await (
    await context.request.get(
      base +
        '/api/board/projects?organization_id=00000000-0000-0000-0000-000000000002'
    )
  ).json();
  const project = projects.data.projects[0];
  result.liveResource = await (
    await context.request.get(base + '/api/resource-observation')
  ).json();
  const page = await context.newPage();
  page.on('pageerror', (e) => result.errors.push(e.message));
  await page.route('**/api/**', (route) =>
    ['GET', 'HEAD'].includes(route.request().method()) ||
    route.request().url().endsWith('/provider-usage/read')
      ? route.fallback()
      : route.fulfill({ json: { success: true, data: null } })
  );
  let language = 'EN';
  await page.route('**/api/info', async (route) => {
    const response = await route.fetch(),
      body = await response.json();
    body.data.config.language = language;
    body.data.config.show_release_notes = false;
    await route.fulfill({ response, json: body });
  });
  const panel = page.getByTestId('environment-panel');
  const goto = async () => {
    await page.goto(base + '/projects/' + project.id);
    await panel.waitFor({ state: 'attached', timeout: 60000 });
    if (!(await panel.isVisible()))
      await page.getByRole('button', { name: /Show Right Sidebar/ }).click();
    await expect(panel.getByTestId('environment-hostname')).not.toHaveText('—');
  };
  const measure = async (name) => {
    const m = await panel.evaluate((el) => {
      const box = (e) => {
        const r = e.getBoundingClientRect();
        return {
          x: r.x,
          y: r.y,
          width: r.width,
          right: r.right,
          bottom: r.bottom,
        };
      };
      return {
        panel: box(el),
        ancestors: [
          el.parentElement,
          el.parentElement.parentElement,
          el.parentElement.parentElement.parentElement,
        ].map((e) => ({
          class: e.className,
          ...box(e),
          overflow: getComputedStyle(e).overflow,
        })),
        cells: [
          ...el
            .querySelector('[data-testid="environment-metrics"]')
            .querySelectorAll('button'),
        ].map((e) => ({
          text: e.innerText,
          cell: box(e),
          meter: box(e.lastElementChild),
          parts: [...e.firstElementChild.children].map((s) => {
            const range = document.createRange();
            range.selectNodeContents(s);
            return { text: s.textContent, ...box(s), glyph: box(range) };
          }),
          scrollWidth: e.scrollWidth,
          clientWidth: e.clientWidth,
        })),
      };
    });
    result.measurements.push({ name, ...m });
    await page.screenshot({ path: out + '/' + name + '.png' });
    return m;
  };
  await goto();
  await page.waitForTimeout(1500);
  await measure('after-live');
  await panel
    .getByRole('button', { name: 'DISK details', exact: true })
    .click();
  result.liveDetails = await page
    .locator('[role="dialog"][data-state="open"]')
    .innerText();
  await page.waitForTimeout(250);
  await page.screenshot({ path: out + '/after-live-disk-details.png' });
  await page.keyboard.press('Escape');
  let mode = 'known',
    repoCount = 0;
  const known = (value) => ({ status: 'known', value });
  await page.route(`**/api/projects/${project.id}/utilities/info`, (route) =>
    route.fulfill({
      json: {
        success: true,
        data: {
          repos: repoCount
            ? [
                {
                  id: 'disk-fixture-repo',
                  name: 'Fixture',
                  path: '/isolated/fixture/repository',
                  wiki_enabled: false,
                },
              ]
            : [],
        },
      },
    })
  );
  await page.route('**/api/resource-observation', (route) => {
    if (mode === 'unavailable') return route.fulfill({ status: 503, json: {} });
    const value = mode === 'zero' ? 0 : 100;
    return route.fulfill({
      json: {
        success: true,
        data: {
          timestamp: new Date(
            Date.now() - (mode === 'stale' ? 60000 : 0)
          ).toISOString(),
          system_visible: {
            cpu_usage: { percent: known(value), sampling_duration_ms: 500 },
            memory_total_bytes: known(1000),
            memory_available_bytes: known(1000 - value * 10),
            swap_total_bytes: known(1000),
            swap_used_bytes: known(value * 10),
          },
          disks:
            mode === 'missing'
              ? []
              : [
                  {
                    target: 'worktree_base',
                    observation:
                      mode === 'unknown'
                        ? { status: 'unknown', error: 'unreadable_destination' }
                        : mode === 'null'
                          ? null
                          : known({
                              total_bytes: 1000e9,
                              used_bytes: value * 10e9,
                              used_percent: value,
                            }),
                  },
                ],
        },
      },
    });
  });
  const constrain = async (width) =>
    panel.evaluate((el, width) => {
      const parent = el.parentElement;
      Object.assign(parent.style, {
        width: width + 'px',
        minWidth: width + 'px',
        maxWidth: width + 'px',
        overflow: 'hidden',
      });
    }, width);
  for (const viewport of [320, 768, 1440])
    for (const theme of ['light', 'dark'])
      for (const count of [0, 1]) {
        repoCount = count;
        mode = 'known';
        await page.setViewportSize({ width: viewport, height: 1000 });
        await goto();
        await expect(
          panel.getByRole('button', { name: 'DISK details', exact: true })
        ).toContainText('100%');
        await page.evaluate((theme) => {
          document.documentElement.classList.toggle('dark', theme === 'dark');
        }, theme);
        for (const width of [280, 300, 320]) {
          await constrain(width);
          const m = await measure(
            `fixture-${viewport}-${theme}-${count}repo-${width}`
          );
          expect(m.panel.width).toBe(width - 1);
          const y = m.cells[0].cell.y;
          for (const cell of m.cells) {
            expect(cell.cell.y).toBe(y);
            expect(cell.scrollWidth).toBeLessThanOrEqual(cell.clientWidth + 1);
            const [label, value] = cell.parts;
            expect(label.right).toBeLessThanOrEqual(value.x);
            for (const part of cell.parts) {
              expect(part.x).toBeGreaterThanOrEqual(cell.cell.x);
              expect(part.right).toBeLessThanOrEqual(cell.cell.right + 0.1);
              expect(part.right).toBeLessThanOrEqual(m.panel.right);
              expect(part.glyph.right).toBeLessThanOrEqual(
                cell.cell.right + 0.1
              );
            }
          }
        }
        for (const label of ['CPU', 'RAM', 'SWAP', 'DISK']) {
          const button = panel.getByRole('button', {
            name: label + ' details',
            exact: true,
          });
          await button.focus();
          await page.keyboard.press('Enter');
          const dialog = page.locator('[role="dialog"][data-state="open"]');
          await expect(dialog).toBeVisible();
          await page.waitForTimeout(250);
          await expect(dialog).toContainText('Observed:');
          if (label !== 'CPU') {
            await expect(dialog).toContainText('Used');
            await expect(dialog).toContainText('Total');
          }
          if (label === 'DISK') {
            await expect(dialog).toContainText('931.32 GiB');
            await expect(dialog).toContainText('Current');
          }
          await page.keyboard.press('Escape');
          await expect(
            page.locator('[role="dialog"][data-state="open"]')
          ).toHaveCount(0);
          await expect(button).toBeFocused();
        }
      }
  result.states = [];
  for (mode of ['zero', 'missing', 'null', 'unknown', 'stale', 'unavailable']) {
    await goto();
    await constrain(300);
    const button = panel.getByRole('button', {
      name: 'DISK details',
      exact: true,
    });
    await expect(button).toContainText(mode === 'zero' ? '0%' : '—');
    await button.focus();
    await page.keyboard.press('Space');
    const dialog = page.locator('[role="dialog"][data-state="open"]');
    await expect(dialog).toBeVisible();
    await page.waitForTimeout(250);
    await expect(dialog).toContainText(
      mode === 'zero'
        ? 'Current'
        : mode === 'stale'
          ? 'Observation expired'
          : mode === 'unavailable'
            ? 'Acquisition failed'
            : mode === 'unknown'
              ? 'storage destination unreadable'
              : 'no valid capacity data'
    );
    const text = await dialog.innerText();
    result.states.push({ mode, text });
    await page.screenshot({ path: out + '/state-' + mode + '.png' });
    await page.keyboard.press('Escape');
  }
  await page.unroute('**/api/resource-observation');
  await page.unroute(`**/api/projects/${project.id}/utilities/info`);
  language = 'JA';
  await page.setViewportSize({ width: 1440, height: 1000 });
  await goto();
  await constrain(300);
  await measure('after-live-ja-300');
  await panel.getByRole('button', { name: 'DISK の詳細', exact: true }).click();
  await page.waitForTimeout(250);
  await page.screenshot({ path: out + '/after-live-ja-300-details.png' });
  expect(result.errors).toEqual([]);
  await writeFile(out + '/verification.json', JSON.stringify(result, null, 2));
  console.log(
    JSON.stringify(
      {
        measurements: result.measurements.length,
        errors: result.errors,
        liveDisk: result.liveResource.data.disks,
        liveDetails: result.liveDetails,
        states: result.states,
      },
      null,
      2
    )
  );
} finally {
  await browser.close();
}
