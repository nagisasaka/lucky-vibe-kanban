// Real React components, browser decoding and ZIP parser; mocked preferences/attachment transport only.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const root = process.cwd(),
  output = path.resolve('test-results/pets'),
  scratch = await mkdtemp(path.join(tmpdir(), 'lvk-pets-'));
await mkdir(output, { recursive: true });
let server, browser;
const errors = [],
  cases = [];
let uploads = 0;
try {
  const bundle = await build({
    entryPoints: ['tests/pets/browser.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    define: { 'import.meta.env.DEV': 'false' },
    alias: {
      '@/shared/stores/useUiPreferencesStore': path.join(
        root,
        'tests/pets/store.fixture.ts'
      ),
      '@/shared/lib/petPreferencesPersistence': path.join(
        root,
        'tests/pets/api.fixture.ts'
      ),
      '@/shared/lib/api': path.join(root, 'tests/pets/api.fixture.ts'),
      '@': path.join(root, 'packages/web-core/src'),
    },
    nodePaths: [path.join(root, 'packages/web-core/node_modules')],
  });
  execFileSync(
    path.join(root, 'packages/local-web/node_modules/.bin/tailwindcss'),
    [
      '-c',
      'tailwind.new.config.js',
      '-i',
      '../web-core/src/app/styles/new/index.css',
      '-o',
      path.join(scratch, 'style.css'),
    ],
    { cwd: path.join(root, 'packages/local-web'), stdio: 'pipe' }
  );
  const css = await readFile(path.join(scratch, 'style.css')),
    atlas = await readFile('packages/public/pets/lvk-rabbit/spritesheet.webp');
  server = createServer((req, res) => {
    if (req.url === '/missing-import.webp') {
      res.writeHead(404);
      res.end();
      return;
    }
    if (req.url === '/fixture-upload') {
      uploads++;
      req.resume();
      res.end('{}');
      return;
    }
    const [mime, body] =
      req.url === '/app.js'
        ? ['text/javascript', bundle.outputFiles[0].text]
        : req.url === '/style.css'
          ? ['text/css', css]
          : req.url === '/pets/lvk-rabbit/spritesheet.webp'
            ? ['image/webp', atlas]
            : [
                'text/html',
                '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script src="/app.js"></script></body></html>',
              ];
    res.setHeader('Content-Type', mime);
    res.end(body);
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    headless: true,
    executablePath: process.env.LVK_E2E_CHROMIUM,
  });
  const page = await browser.newPage({
    viewport: { width: 1200, height: 780 },
  });
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route('**/*', (r) =>
    new URL(r.request().url()).origin === base ? r.continue() : r.abort()
  );
  await page.goto(base);
  const sidebar = page.getByTestId('pet-settings'),
    pet = sidebar.getByRole('button', { name: 'LVK Rabbit', exact: true });
  await expect(pet).toBeVisible();
  await expect(pet).toHaveCSS('width', '80px');
  await expect(pet).toHaveCSS('height', '80px');
  await expect(pet.locator('img')).toHaveCSS('height', '720px');
  await expect(pet.locator('img')).toHaveJSProperty('naturalWidth', 1536);
  const states = [
    'idle',
    'running-right',
    'running-left',
    'waving',
    'jumping',
    'failed',
    'waiting',
    'running',
    'review',
  ];
  for (const [row, state] of states.entries()) {
    const sprite = page.getByRole('button', { name: state, exact: true });
    await expect(sprite).toHaveAttribute('data-pet-state', state);
    await expect(sprite).toHaveCSS('width', '64px');
    await expect(sprite).toHaveCSS('height', '64px');
    await expect(sprite.locator('img')).toHaveCSS('top', `${-row * 64}px`);
    await expect
      .poll(() => sprite.getAttribute('data-pet-frame'))
      .not.toBe('0');
  }
  cases.push(
    'nine states: image decode, correct row and real timer frame progression'
  );
  for (const [row, state] of states.entries()) {
    await sidebar.getByLabel('Animation preview').selectOption(state);
    await expect(pet.locator('img')).toHaveCSS('top', `${-row * 80}px`);
  }
  await sidebar.getByLabel('Animation preview').selectOption('idle');
  cases.push(
    'sidebar body 80px; nine rows scaled consistently; default sprites stay 64px'
  );
  await pet.click();
  await expect(pet).toHaveAttribute('data-pet-state', 'waving');
  await expect(pet).toHaveAttribute('data-pet-state', 'idle', {
    timeout: 2000,
  });
  cases.push('click waving reaction returns to idle');
  for (const dark of [false, true]) {
    await page.evaluate(
      (d) => document.documentElement.classList.toggle('dark', d),
      dark
    );
    await page.screenshot({
      path: path.join(output, dark ? 'dark.png' : 'light.png'),
    });
  }
  await page.emulateMedia({ reducedMotion: 'reduce' });
  for (const state of states)
    await expect(
      page.getByRole('button', { name: state, exact: true })
    ).toHaveAttribute('data-pet-frame', '0');
  await page.waitForTimeout(400);
  await expect(pet).toHaveAttribute('data-pet-frame', '0');
  cases.push('reduced motion stops all nine at frame zero');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  const show = sidebar.getByRole('checkbox');
  await show.uncheck();
  await expect(pet).toHaveCount(0);
  await page.reload();
  await expect(sidebar.locator('[data-pet-state]')).toHaveCount(0);
  await sidebar.getByRole('button', { name: 'Restore LVK Rabbit' }).click();
  await expect(pet).toBeVisible();
  cases.push('hidden preference survives reload; reset restores standard pet');
  const zip = path.join(scratch, 'pet.zip');
  execFileSync('python3', [
    '-c',
    `import zipfile,sys,json\nfrom pathlib import Path\np=Path(sys.argv[1]);o=Path(sys.argv[2])\nm=json.loads((p/'pet.json').read_text());m['id']='fixture-rabbit';m['displayName']='Imported fixture rabbit';m['author']='Fixture author';m['license']='CC BY 4.0'\nwith zipfile.ZipFile(o,'w',zipfile.ZIP_DEFLATED) as z:\n z.writestr('pet.json',json.dumps(m));z.write(p/'spritesheet.webp','spritesheet.webp');z.write(p/'LICENSE','LICENSE')`,
    'packages/public/pets/lvk-rabbit',
    zip,
  ]);
  await sidebar.locator('input[type=file]').setInputFiles(zip);
  const imported = sidebar.getByRole('button', {
    name: 'Imported fixture rabbit',
  });
  await expect(imported).toBeVisible();
  assert.equal(uploads, 1);
  await page.reload();
  await expect(imported).toBeVisible();
  await expect(sidebar.locator('select').first()).toHaveValue(
    /^[0-9a-f-]{36}$/
  );
  await sidebar.locator('select').first().selectOption('lvk-rabbit');
  await expect(pet).toBeVisible();
  await sidebar
    .locator('select')
    .first()
    .selectOption({ label: 'Imported fixture rabbit' });
  await expect(imported).toBeVisible();
  cases.push(
    'real v1 ZIP decode/import, mocked upload, selection and reload restoration'
  );
  await sidebar.locator('input[type=file]').setInputFiles({
    name: 'broken.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from('not zip'),
  });
  await expect(sidebar.getByRole('status')).not.toBeEmpty();
  await expect(imported).toBeVisible();
  assert.equal(uploads, 1);
  cases.push(
    'broken ZIP reports error without losing selected pet or uploading'
  );
  await page.getByRole('button', { name: 'Toggle sidebar' }).click();
  await expect(sidebar).toHaveCount(0);
  await page.getByRole('button', { name: 'Toggle sidebar' }).click();
  await expect(imported).toBeVisible();
  cases.push('sidebar unmount removes pet; remount keeps selection');
  await page.evaluate(() => localStorage.setItem('pet-save-failure', '1'));
  await sidebar.getByRole('button', { name: 'Restore LVK Rabbit' }).click();
  await expect(sidebar.getByRole('status')).toContainText(
    'Could not load or save'
  );
  await expect(imported).toBeVisible();
  await page.reload();
  await expect(imported).toBeVisible();
  await page.evaluate(() => localStorage.removeItem('pet-save-failure'));
  cases.push(
    'failed settings persistence reports error and retains the previous selection'
  );
  await page.evaluate(() => localStorage.setItem('pet-image-failure', '1'));
  await page.reload();
  await expect(pet).toBeVisible();
  await expect(imported).toHaveCount(0);
  await expect(sidebar).toContainText(
    'Showing LVK Rabbit because Imported fixture rabbit could not be loaded'
  );
  await expect(sidebar).toContainText('LVK Rabbit — Masato Fujino');
  await expect(sidebar.locator('select').first()).toHaveValue(
    /^[0-9a-f-]{36}$/
  );
  const credits = sidebar.getByText(
    'Retained selection metadata / license (not displayed)',
    { exact: true }
  );
  await credits.click();
  await expect(sidebar).toContainText('Fixture author');
  cases.push(
    'missing imported image displays Rabbit name and attribution, retaining separate selected-pet credits'
  );

  await page.evaluate(() => localStorage.removeItem('pet-image-failure'));
  await page.reload();
  // Generated geometry is a decoder/coordinate fixture, not pet artwork.
  const makeAtlas = async (
    mime = 'image/png',
    missing = false,
    paintedTail = false
  ) =>
    page.evaluate(
      ({ mime, missing, paintedTail }) => {
        const canvas = document.createElement('canvas');
        canvas.width = 1536;
        canvas.height = 2288;
        const ctx = canvas.getContext('2d');
        const counts = [6, 8, 8, 4, 5, 8, 6, 6, 6, 8, 8];
        for (let row = 0; row < 11; row++)
          for (let col = 0; col < counts[row]; col++) {
            if (missing && row === 10 && col === 7) continue;
            ctx.save();
            ctx.translate(col * 192 + 96, row * 208 + 104);
            ctx.fillStyle = row < 9 ? `hsl(${row * 35},65%,45%)` : '#2877cc';
            ctx.fillRect(-34, -40, 68, 80);
            if (row >= 9) {
              ctx.rotate((((row - 9) * 8 + col) * Math.PI) / 8);
              ctx.fillStyle = '#ffffff';
              ctx.beginPath();
              ctx.moveTo(0, -32);
              ctx.lineTo(18, 12);
              ctx.lineTo(0, 3);
              ctx.lineTo(-18, 12);
              ctx.closePath();
              ctx.fill();
            }
            ctx.restore();
          }
        if (paintedTail) {
          ctx.fillStyle = '#ff0000';
          ctx.fillRect(7 * 192, 0, 1, 1);
        }
        return canvas.toDataURL(mime, 1);
      },
      { mime, missing, paintedTail }
    );
  const v2Manifest = {
    id: 'v2-fixture',
    displayName: 'V2 direction fixture',
    description: 'Generated geometric test pattern; not user artwork',
    spritesheetPath: 'sheet.png',
    spriteVersionNumber: 2,
  };
  const packAtlas = async (name, manifest, dataUrl) => {
    const image = path.join(scratch, name + '.image'),
      zipPath = path.join(scratch, name + '.zip');
    await writeFile(image, Buffer.from(dataUrl.split(',')[1], 'base64'));
    execFileSync('python3', [
      '-c',
      'import zipfile,json,sys\nwith zipfile.ZipFile(sys.argv[1],"w",zipfile.ZIP_DEFLATED) as z:\n z.writestr("pet.json",sys.argv[2]);z.write(sys.argv[3],json.loads(sys.argv[2])["spritesheetPath"])',
      zipPath,
      JSON.stringify(manifest),
      image,
    ]);
    return zipPath;
  };
  const body = page.getByTestId('sidebar-pet').getByRole('button');
  // Legacy saved v1s were accepted by dimensions/decode, not cell occupancy.
  // Keep a visible idle cell, paint an unused tail, leave other cells empty.
  const legacyPng = await page.evaluate(() => {
    const c = document.createElement('canvas');
    c.width = 1536;
    c.height = 1872;
    const ctx = c.getContext('2d');
    ctx.fillStyle = '#e47c29';
    ctx.fillRect(60, 50, 72, 100);
    ctx.fillRect(7 * 192, 0, 1, 1);
    return c.toDataURL('image/png');
  });
  const legacyManifest = {
    id: 'legacy-v1',
    displayName: 'Legacy v1 fixture',
    description: '',
    spritesheetPath: 'sheet.png',
  };
  await sidebar
    .locator('input[type=file]')
    .setInputFiles(await packAtlas('legacy-v1', legacyManifest, legacyPng));
  const legacyPet = sidebar.getByRole('button', {
    name: legacyManifest.displayName,
    exact: true,
  });
  await expect(legacyPet).toHaveAttribute('data-pet-version', '1');
  await page.reload();
  await expect(legacyPet).toHaveAttribute('data-pet-version', '1');
  await expect(body.locator('img')).toHaveJSProperty('naturalHeight', 1872);
  await expect(sidebar.getByRole('status')).toBeEmpty();
  const restoredVersion = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem('pet-fixture')).pets.find(
        (p) => p.manifest.id === 'legacy-v1'
      ).manifest.spriteVersionNumber
  );
  assert.equal(restoredVersion, undefined);
  cases.push(
    'legacy saved v1 without version: empty required cells and painted tail remain accepted after import and reload, no Rabbit fallback'
  );
  // Exact producer occupancy count in the optional neutral cell, with safe
  // geometric content only. 192*112 + 78 = 21,582 nontransparent pixels.
  const neutralPng = await page.evaluate(
    async (source) => {
      const image = await createImageBitmap(await (await fetch(source)).blob());
      const canvas = document.createElement('canvas');
      canvas.width = 1536;
      canvas.height = 2288;
      const ctx = canvas.getContext('2d');
      ctx.drawImage(image, 0, 0);
      image.close();
      ctx.fillStyle = '#2877cc';
      ctx.fillRect(6 * 192, 0, 192, 112);
      ctx.fillRect(6 * 192, 112, 78, 1);
      return canvas.toDataURL('image/png');
    },
    await makeAtlas()
  );
  for (const encoding of ['png', 'webp']) {
    const data =
      encoding === 'png'
        ? neutralPng
        : await page.evaluate(async (source) => {
            const image = await createImageBitmap(
              await (await fetch(source)).blob()
            );
            const canvas = document.createElement('canvas');
            canvas.width = 1536;
            canvas.height = 2288;
            canvas.getContext('2d').drawImage(image, 0, 0);
            image.close();
            return canvas.toDataURL('image/webp', 1);
          }, neutralPng);
    const name = `V2 neutral ${encoding} fixture`;
    await sidebar
      .locator('input[type=file]')
      .setInputFiles(
        await packAtlas(
          `v2-neutral-${encoding}`,
          {
            ...v2Manifest,
            displayName: name,
            spritesheetPath: `sheet.${encoding}`,
          },
          data
        )
      );
    await expect(
      sidebar.getByRole('button', { name, exact: true })
    ).toHaveAttribute('data-pet-version', '2');
    await page.reload();
    await expect(
      sidebar.getByRole('button', { name, exact: true })
    ).toHaveAttribute('data-pet-version', '2');
  }
  cases.push(
    'producer-style v2 optional neutral cell row0/col6 with21,582 occupied pixels imports and restores as PNG/WebP; idle keeps six frames, other tails still rejected'
  );
  const v2Png = await makeAtlas();
  await sidebar
    .locator('input[type=file]')
    .setInputFiles(await packAtlas('v2-png', v2Manifest, v2Png));
  const v2 = sidebar.getByRole('button', {
    name: v2Manifest.displayName,
    exact: true,
  });
  await expect(v2).toHaveAttribute('data-pet-version', '2');
  await expect(body).toHaveAttribute('data-pet-version', '2');
  await expect(v2.locator('img')).toHaveCSS('height', '880px');
  await expect(v2.locator('img')).toHaveJSProperty('naturalHeight', 2288);
  for (let index = 0; index < 16; index++) {
    const bounds = await v2.boundingBox(),
      angle = (index * Math.PI) / 8;
    await page.mouse.move(
      bounds.x + 40 + Math.sin(angle) * 48,
      bounds.y + 40 - Math.cos(angle) * 48
    );
    await expect(v2).toHaveAttribute('data-pet-direction', String(index));
    await expect(v2.locator('img')).toHaveCSS(
      'top',
      `${-(9 + Math.floor(index / 8)) * 80}px`
    );
    await expect(v2).toHaveAttribute('data-pet-frame', String(index % 8));
    if (index % 4 === 0)
      await v2.screenshot({ path: path.join(output, `v2-look-${index}.png`) });
  }
  await page.waitForTimeout(350);
  await expect(v2).toHaveAttribute('data-pet-direction', '15');
  await expect(v2).toHaveAttribute('data-pet-frame', '7');
  const bounds = await v2.boundingBox();
  await page.mouse.move(bounds.x + 40, bounds.y + 40);
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await page.mouse.move(bounds.x + 40, bounds.y - 8);
  await expect(v2).toHaveAttribute('data-pet-direction', '0');
  await page.evaluate(() =>
    document.dispatchEvent(
      new PointerEvent('pointerout', { relatedTarget: null })
    )
  );
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await page.mouse.move(bounds.x + 88, bounds.y + 40);
  await expect(v2).toHaveAttribute('data-pet-direction', '4');
  await page.evaluate(() =>
    window.dispatchEvent(
      new PointerEvent('pointerdown', {
        pointerType: 'touch',
        clientX: 50,
        clientY: 50,
      })
    )
  );
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await v2.click();
  await expect(v2).toHaveAttribute('data-pet-state', 'waving');
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await expect(v2.locator('img')).toHaveCSS('top', '-240px');
  await expect(v2).toHaveAttribute('data-pet-state', 'idle', { timeout: 2000 });
  await sidebar.getByLabel('Animation preview').selectOption('running');
  await page.mouse.move(bounds.x + 88, bounds.y + 40);
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await expect(v2.locator('img')).toHaveCSS('top', '-560px');
  await sidebar.getByLabel('Animation preview').selectOption('idle');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.mouse.move(bounds.x + 40, bounds.y - 8);
  await expect(v2).not.toHaveAttribute('data-pet-direction');
  await expect(v2).toHaveAttribute('data-pet-frame', '0');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  cases.push(
    'v2 all 16 clockwise static cells; north is up, center/leave/touch/reduced motion return to idle; reaction and state animation take priority'
  );
  await page.reload();
  await expect(v2).toHaveAttribute('data-pet-version', '2');
  await expect(body).toHaveAttribute('data-pet-version', '2');
  await sidebar.locator('select').first().selectOption('lvk-rabbit');
  await expect(pet).toHaveAttribute('data-pet-version', '1');
  await expect(pet.locator('img')).toHaveCSS('height', '720px');
  await sidebar
    .locator('select')
    .first()
    .selectOption({ label: v2Manifest.displayName });
  await expect(v2).toHaveAttribute('data-pet-version', '2');
  cases.push(
    'v2 settings and sidebar restore version on reload and v1/v2 selection switches preserve atlas scaling'
  );
  const beforeInvalid = uploads;
  for (const [name, manifest, data] of [
    ['missing-look', v2Manifest, await makeAtlas('image/png', true)],
    ['painted-tail', v2Manifest, await makeAtlas('image/png', false, true)],
    ['unknown-version', { ...v2Manifest, spriteVersionNumber: 3 }, v2Png],
    [
      'missing-version',
      { ...v2Manifest, spriteVersionNumber: undefined },
      v2Png,
    ],
  ]) {
    await sidebar
      .locator('input[type=file]')
      .setInputFiles(await packAtlas(name, manifest, data));
    await expect(sidebar.getByRole('status')).not.toBeEmpty();
    await expect(v2).toBeVisible();
  }
  assert.equal(uploads, beforeInvalid);
  cases.push(
    'empty look cell, painted unused tail, unknown/missing version reject before upload and retain selection'
  );
  const v2Webp = await makeAtlas('image/webp');
  await sidebar.locator('input[type=file]').setInputFiles(
    await packAtlas(
      'v2-webp',
      {
        ...v2Manifest,
        id: 'v2-webp',
        displayName: 'V2 WebP fixture',
        spritesheetPath: 'sheet.webp',
      },
      v2Webp
    )
  );
  await expect(
    sidebar.getByRole('button', { name: 'V2 WebP fixture' })
  ).toHaveAttribute('data-pet-version', '2');
  await page.reload();
  await expect(body.locator('img')).toHaveJSProperty('naturalHeight', 2288);
  cases.push(
    'actual browser WebP encoding, ZIP import/decoded dimensions and reload restoration'
  );
  await page.evaluate(
    (invalidImage) => localStorage.setItem('pet-fixture-image', invalidImage),
    await makeAtlas('image/png', true)
  );
  await page.reload();
  await expect(
    sidebar.getByRole('button', { name: 'LVK Rabbit', exact: true })
  ).toBeVisible();
  await expect(sidebar.locator('select').first()).toHaveValue(
    /^[0-9a-f-]{36}$/
  );
  await expect(sidebar.getByRole('status')).not.toBeEmpty();
  await page.evaluate(
    (validImage) => localStorage.setItem('pet-fixture-image', validImage),
    v2Webp
  );
  await page.reload();
  await expect(body).toHaveAttribute('data-pet-version', '2');
  cases.push(
    'restored malformed v2 atlas falls back to Rabbit while keeping selection/metadata; corrected image reload restores v2'
  );
  for (const lang of ['en', 'ja'])
    for (const width of [360, 1200]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(base + '?lang=' + lang);
      await expect(body).toBeVisible();
      await page.evaluate(
        (dark) => document.documentElement.classList.toggle('dark', dark),
        width === 360
      );
      await page.waitForTimeout(300);
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth
        ),
        false
      );
      const panel = await page.getByTestId('sidebar-pet').evaluate((el) => ({
        panel: el.parentElement.getBoundingClientRect().width,
        pet: el.getBoundingClientRect().width,
      }));
      assert.ok(panel.pet <= panel.panel);
      await page.screenshot({
        path: path.join(output, `v2-${lang}-${width}.png`),
        fullPage: true,
      });
    }
  cases.push(
    'v2 settings/sidebar screenshots: en/ja, 360px dark and desktop light; no horizontal overflow'
  );
  // Exercise actual file change event and asynchronous feedback, not just parser results.
  await page.setViewportSize({ width: 360, height: 700 });
  await page.goto(base + '?lang=en');
  let releaseUpload;
  const uploadGate = new Promise((resolve) => {
    releaseUpload = resolve;
  });
  await page.route(
    '**/fixture-upload',
    async (route) => {
      await uploadGate;
      await route.continue();
    },
    { times: 1 }
  );
  const fileInput = sidebar.locator('input[type=file]');
  const chooserReady = page.waitForEvent('filechooser');
  await fileInput.click();
  const chooser = await chooserReady;
  await chooser.setFiles(path.join(scratch, 'v2-png.zip'));
  const feedback = sidebar.getByRole('status');
  await expect(feedback).toContainText('Importing');
  await expect(feedback).toContainText('v2-png.zip');
  await expect(fileInput).toBeDisabled();
  const nearby = await feedback.evaluate((el) => {
    const input = document.querySelector('input[type=file]');
    return {
      gap:
        el.getBoundingClientRect().top - input.getBoundingClientRect().bottom,
      described: input.getAttribute('aria-describedby') === el.id,
    };
  });
  assert.ok(nearby.gap >= 0 && nearby.gap < 60);
  assert.equal(nearby.described, true);
  releaseUpload();
  await expect(feedback).toContainText('Imported V2 direction fixture.');
  await expect(fileInput).toBeEnabled();
  await page.route(
    '**/fixture-upload',
    (route) => route.fulfill({ status: 500, body: 'fixture failure' }),
    { times: 1 }
  );
  const selectionBeforeFailure = await sidebar
    .locator('select')
    .first()
    .inputValue();
  await fileInput.setInputFiles(path.join(scratch, 'v2-png.zip'));
  await expect(feedback).toContainText('Could not load or save');
  await expect(sidebar.locator('select').first()).toHaveValue(
    selectionBeforeFailure
  );
  await expect(feedback).not.toContainText('Imported');
  await expect(fileInput).toBeEnabled();
  await fileInput.setInputFiles({
    name: 'invalid.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from('invalid'),
  });
  await expect(feedback).toContainText('Invalid or damaged');
  await expect(feedback).toContainText('invalid.zip');
  await page.screenshot({
    path: path.join(output, 'zip-feedback-360.png'),
    fullPage: true,
  });
  cases.push(
    'file input change: nearby live feedback and filename during pending upload, durable success, HTTP failure and invalid ZIP; selection retained and retry enabled'
  );
  const uploadsBeforeLimit = uploads;
  await fileInput.setInputFiles({
    name: 'lvk-rabbit-depth-b-v2-delivery.zip',
    mimeType: 'application/zip',
    buffer: Buffer.alloc(14512968),
  });
  await expect(feedback).toContainText('ZIP is too large');
  await expect(feedback).toContainText('14,512,968');
  await expect(feedback).toContainText('8,388,608');
  assert.equal(uploads, uploadsBeforeLimit);
  await page.screenshot({
    path: path.join(output, 'zip-size-limit-360.png'),
    fullPage: true,
  });
  await page.evaluate(() => {
    const p = JSON.parse(localStorage.getItem('pet-fixture'));
    const source = p.pets.find((pet) => pet.manifest.spriteVersionNumber === 2);
    p.pets = Array.from({ length: 12 }, () => ({
      ...source,
      key: crypto.randomUUID(),
    }));
    p.selected = p.pets[0].key;
    localStorage.setItem('pet-fixture', JSON.stringify(p));
  });
  await page.reload();
  await sidebar.locator('input[type=file]').setInputFiles({
    name: 'attempt.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from('not read when collection full'),
  });
  await expect(sidebar.getByRole('status')).toContainText(
    'collection is full (12 of 12 pets)'
  );
  await expect(sidebar.getByRole('status')).not.toContainText(
    'ZIP is too large'
  );
  assert.equal(uploads, uploadsBeforeLimit);
  cases.push(
    'reported 14,512,968-byte metadata reproduces pre-read ZIP size refusal with numeric limit; isolated 12-pet collection gives a separate error and neither uploads'
  );
  await page.goto(base + '?lang=ja');
  await sidebar.locator('input[type=file]').setInputFiles({
    name: 'attempt.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from('not read'),
  });
  await expect(sidebar.getByRole('status')).toContainText('保存済みPetが上限');
  await page.evaluate(() => localStorage.removeItem('pet-fixture'));
  await page.reload();
  await sidebar.locator('input[type=file]').setInputFiles({
    name: 'lvk-rabbit-depth-b-v2-delivery.zip',
    mimeType: 'application/zip',
    buffer: Buffer.alloc(14512968),
  });
  await expect(sidebar.getByRole('status')).toContainText(
    'ZIPファイルが大きすぎます'
  );
  await expect(sidebar.getByRole('status')).toContainText('14,512,968');
  await expect(sidebar.getByRole('status')).toContainText('8,388,608');
  await page.screenshot({
    path: path.join(output, 'zip-size-limit-ja-360.png'),
    fullPage: true,
  });
  assert.equal(uploads, uploadsBeforeLimit);

  assert.deepEqual(errors, []);
  console.log(
    JSON.stringify({ status: 'passed', cases, errors, uploads }, null, 2)
  );
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify({ status: 'passed', cases, errors, uploads }, null, 2)
  );
} finally {
  await browser?.close();
  if (server) await new Promise((r) => server.close(r));
  await rm(scratch, { recursive: true, force: true });
}
