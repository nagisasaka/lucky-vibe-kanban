// Exercise the real Project utility layout with inert file/wiki/terminal providers.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, rm, mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
const root = process.cwd(),
  out = path.resolve('test-results/utility-sidebar');
const scratch = await mkdtemp(path.join(tmpdir(), 'lvk-sidebar-'));
const require = createRequire(
  path.join(root, 'packages/web-core/package.json')
);
const ts = require('typescript');
await mkdir(out, { recursive: true });
const source = await readFile(
  'packages/web-core/src/pages/kanban/ProjectUtilities.tsx',
  'utf8'
);
const ast = ts.createSourceFile(
  'ProjectUtilities.tsx',
  source,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX
);
const component = ast.statements
  .find(
    (n) => ts.isFunctionDeclaration(n) && n.name?.text === 'ProjectUtilities'
  )
  .getText(ast);
const stub = path.join(scratch, 'environment.tsx');
await writeFile(
  stub,
  'export const EnvironmentPanel=()=> <div style={{height:100}}>Environment</div>;'
);
const fixture = `
import React from 'react';import {createRoot} from 'react-dom/client';
import {UtilitySidebar} from './packages/web-core/src/shared/components/UtilitySidebar';
import {CollapsibleSectionHeader} from './packages/ui/src/components/CollapsibleSectionHeader';
import {useUiPreferencesStore} from './packages/web-core/src/shared/stores/useUiPreferencesStore';
const useTranslation=()=>({t:key=>key});const FolderSimpleIcon=()=>null,CaretDownIcon=()=>null;
const mainTerminalRepo=repos=>repos[0];
const rows=Number(new URLSearchParams(location.search).get('rows'))||2;
const WorkspaceFileBrowserPanel=()=> <div className="flex h-full min-h-0 flex-col"><div className="shrink-0 p-base"><input aria-label="Search files"/></div><div data-file-tree-scroll className="min-h-0 flex-1 overflow-auto p-base">{Array.from({length:rows},(_,i)=><div key={i} style={{height:30}}>file-{i}.ts</div>)}</div></div>;
const WorkspaceWikiNavigation=()=> <div className="flex h-full min-h-0 flex-1 flex-col"><div className="shrink-0 p-base">Wiki controls</div><nav className="min-h-0 flex-1 overflow-auto">{Array.from({length:100},(_,i)=><p key={i}>Page {i}</p>)}</nav></div>;
const TerminalSection=()=> <CollapsibleSectionHeader className="!h-auto" title="Terminal" defaultExpanded={false}><div className="h-80">Inert terminal</div></CollapsibleSectionHeader>;
${component}
createRoot(document.getElementById('root')).render(<div style={{height:'100vh',width:320}}><ProjectUtilities projectId="fixture" repos={[{id:'repo',name:'Fixture',path:'/fixture',wiki_enabled:true}]} error={false} selectedFile={null} onFile={()=>{}} onWiki={()=>{}} onClose={()=>{}}/></div>);`;
let browser;
const results = [];
try {
  const js = await build({
    stdin: { contents: fixture, loader: 'tsx', resolveDir: root },
    bundle: true,
    outdir: scratch,
    write: false,
    jsx: 'automatic',
    format: 'iife',
    alias: {
      '@': path.join(root, 'packages/web-core/src'),
    },
    plugins: [
      {
        name: 'inert-environment',
        setup(build) {
          build.onResolve({ filter: /EnvironmentPanel$/ }, () => ({
            path: stub,
          }));
        },
      },
    ],
    nodePaths: [path.join(root, 'packages/web-core/node_modules')],
    loader: { '.svg': 'dataurl', '.png': 'dataurl' },
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
  const css = await readFile(path.join(scratch, 'style.css'), 'utf8');
  const layoutCss = js.outputFiles.find((f) => f.path.endsWith('.css')).text;
  const code = js.outputFiles.find((f) => f.path.endsWith('.js')).text;
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.route('**/*', (r) =>
    r.fulfill({
      contentType: 'text/html',
      body: `<!doctype html><style>${css}\n${layoutCss}</style><div id="root"></div><script>${code}</script>`,
    })
  );
  for (const rows of [2, 120])
    for (const height of [1000, 650, 350]) {
      await page.setViewportSize({ width: 1000, height });
      await page.goto(`https://isolated.invalid/?rows=${rows}`);
      const files = page.getByRole('button', {
          name: 'environment.files',
          exact: true,
        }),
        wiki = page.getByRole('button', {
          name: 'environment.wiki',
          exact: true,
        }),
        terminal = page.getByRole('button', { name: 'Terminal', exact: true });
      await expect(files).toBeVisible();
      const tree = page.locator('[data-file-tree-scroll]');
      if (height >= 650) {
        const b = await wiki.boundingBox();
        assert.ok(
          Math.abs(b.y + b.height - height) < 2,
          'collapsed Wiki stays at bottom'
        );
      }
      if (rows === 120)
        assert.ok(
          await tree.evaluate((e) => e.scrollHeight > e.clientHeight),
          'long tree scrolls internally'
        );
      for (let i = 0; i < 2; i++) {
        await files.click();
        await expect(tree).toHaveCount(0);
        await files.click();
      }
      await wiki.click();
      await expect(wiki).toHaveAttribute('aria-expanded', 'true');
      await terminal.click();
      await expect(page.getByText('Inert terminal')).toHaveCSS(
        'height',
        '320px'
      );
      for (const b of [files, terminal, wiki]) {
        await b.scrollIntoViewIfNeeded();
        await expect(b).toBeInViewport();
      }
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollHeight > innerHeight
        ),
        false,
        'no document overflow'
      );
      results.push({ rows, height, status: 'passed' });
    }
  await writeFile(
    path.join(out, 'results.json'),
    JSON.stringify(results, null, 2)
  );
  console.log(JSON.stringify(results));
} finally {
  await browser?.close();
  await rm(scratch, { recursive: true, force: true });
}
