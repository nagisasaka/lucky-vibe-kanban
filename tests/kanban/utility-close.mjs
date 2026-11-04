// Execute the actual layout functions and existing preview X with isolated provider seams.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, writeFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
const root = process.cwd(),
  output = path.resolve('test-results/kanban-utility-close');
await mkdir(output, { recursive: true });
const scratch = await mkdtemp(path.join(tmpdir(), 'lvk-utility-close-'));
const require = createRequire(
  path.join(root, 'packages/web-core/package.json')
);
const ts = require('typescript');
const source = await readFile(
  'packages/web-core/src/pages/kanban/ProjectKanban.tsx',
  'utf8'
);
const ast = ts.createSourceFile(
  'ProjectKanban.tsx',
  source,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX
);
const names = [
  'getClampedKanbanLeftPanelSize',
  'ProjectKanbanLayout',
  'ProjectKanbanPanes',
];
const parts = ast.statements
  .filter(
    (node) => ts.isFunctionDeclaration(node) && names.includes(node.name?.text)
  )
  .map((node) => node.getText(ast))
  .join('\n');
assert.equal(
  names.filter((name) => parts.includes(`function ${name}`)).length,
  3
);
const stub = path.join(scratch, 'hooks.ts');
await writeFile(
  stub,
  `export const useTheme=()=>({theme:'light'});export const useHostId=()=>null;export const useWorkspaceRecord=()=>({data:null});export const useOpenInEditor=()=>()=>{};`
);
const fixture = `
import {useState,useEffect,useMemo} from 'react';
import {createRoot} from 'react-dom/client';
import {Group,Panel,Separator} from 'react-resizable-panels';
import i18n from 'i18next';import {initReactI18next,useTranslation} from 'react-i18next';
import en from './packages/web-core/src/i18n/locales/en/common.json';
i18n.use(initReactI18next).init({lng:'en',resources:{en:{common:en}},initImmediate:false});
import {WorkspaceFilePreviewHeader} from './packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewHeader';
import {useUiPreferencesStore} from './packages/web-core/src/shared/stores/useUiPreferencesStore';
const useProjectContext=()=>({projectId:'fixture',getIssue:()=>null});
const useCurrentKanbanRouteState=()=>({issueId:undefined,isPanelOpen:false});
const useTodoViewStore=()=>false;const usePageTitle=()=>{};
const useIsMobile=()=>matchMedia('(max-width:767px)').matches;
const usePaneSize=()=>[75,()=>{}];const PERSIST_KEYS={};
const useQuery=()=>({data:{repos:[]},isError:false});const projectUtilitiesApi=()=>({});
const readUtility=()=>{};const FileApiProvider=({children})=>children;
const WorkspaceWikiProvider=FileApiProvider;const WorkspaceWikiArticle=()=>null;
const ProjectRightSidebarContainer=()=>null;const ProjectKanbanBoard=()=> <p>Fixture board</p>;
const ProjectUtilities=({onFile})=><button onClick={()=>onFile({workspaceId:'fixture',repoId:'repo',path:'example.ts',source:'file-tree'})}>Open fixture file</button>;
const WorkspaceFilePreviewPanel=({target,onClearTarget})=><div className="h-full bg-primary"><WorkspaceFilePreviewHeader target={target} onRefresh={()=>{}} onClearTarget={onClearTarget}/><p>Generated file body</p></div>;
const KANBAN_DEFAULT_LEFT_PANEL_SIZE=75,KANBAN_BOARD_MIN_PERCENT=35,KANBAN_BOARD_MIN_SIZE='35%',KANBAN_RIGHT_PANEL_MIN_SIZE='25%';
${parts}
createRoot(document.getElementById('root')).render(<div style={{height:'100vh'}}><button style={{position:'fixed',bottom:0,left:0,zIndex:99}} onClick={()=>useUiPreferencesStore.getState().toggleRightSidebar()}>Open utilities</button><ProjectKanbanLayout projectName="Fixture"/></div>);
`;
let browser;
const result = { cases: [], errors: [] };
try {
  const alias = { '@': path.join(root, 'packages/web-core/src') };
  for (const name of [
    'shared/hooks/useTheme',
    'shared/hooks/useWorkspaceRecord',
    'shared/hooks/useOpenInEditor',
    'shared/providers/HostIdProvider',
  ])
    alias['@/' + name] = stub;
  const js = await build({
    stdin: { contents: fixture, loader: 'tsx', resolveDir: root },
    bundle: true,
    write: false,
    jsx: 'automatic',
    format: 'iife',
    alias,
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
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  page.on('pageerror', (e) => result.errors.push(e.message));
  await page.route('**/*', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>${css}</style><div id="root"></div><script>${js.outputFiles[0].text}</script>`,
    })
  );
  for (const width of [1440, 320]) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto('https://isolated.invalid/');
    for (let repeat = 0; repeat < 2; repeat++) {
      if (
        !(await page
          .getByRole('button', { name: 'Open fixture file', exact: true })
          .isVisible())
      )
        await page
          .getByRole('button', { name: 'Open utilities', exact: true })
          .click();
      await page
        .getByRole('button', { name: 'Open fixture file', exact: true })
        .click();
      await expect(page.getByText('Generated file body')).toBeVisible();
      await expect(
        page.getByRole('button', { name: 'Close', exact: true })
      ).toHaveCount(0);
      const close = page.getByRole('button', {
        name: 'Close preview',
        exact: true,
      });
      await expect(close).toBeVisible();
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth
        ),
        false
      );
      await page.screenshot({
        path: path.join(output, `file-${width}-${repeat}.png`),
      });
      await close.click();
      await expect(page.getByText('Generated file body')).toHaveCount(0);
    }
    result.cases.push(
      `actual layout + existing preview X: close/reopen twice, ${width}px`
    );
  }
  assert.deepEqual(result.errors, []);
  result.status = 'passed';
} catch (error) {
  result.status = 'failed';
  result.error = error.stack;
  throw error;
} finally {
  await browser?.close();
  await rm(scratch, { recursive: true, force: true });
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(result, null, 2)
  );
  console.log(JSON.stringify(result, null, 2));
}
