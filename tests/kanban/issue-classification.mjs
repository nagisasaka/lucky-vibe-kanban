// Component-local Chromium regression; no server, database, or shared device.
import assert from 'node:assert/strict';
import path from 'node:path';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';

const root = process.cwd();
const source = (file) => JSON.stringify(path.join(root, file));
const { outputFiles } = await build({
  stdin: {
    contents: `
      import React, { useState } from 'react';
      import { createRoot } from 'react-dom/client';
      import { IssueTagsRow } from ${source('packages/ui/src/components/IssueTagsRow.tsx')};
      import { SearchableTagDropdownContainer } from ${source('packages/web-core/src/shared/components/SearchableTagDropdownContainer.tsx')};
      import { createBlankCreateFormData, createInitialKanbanIssuePanelFormState, selectDisplayData } from ${source('packages/web-core/src/pages/kanban/kanban-issue-panel-state.ts')};
      import { buildPrioritySelectionPages } from ${source('packages/web-core/src/shared/dialogs/command-bar/selections/prioritySelection.ts')};
      const tags = [
        {id:'system',name:'system',color:'0 0% 50%'},
        {id:'bug',name:'bug',color:'355 65% 53%'},
        {id:'feature',name:'feature',color:'124 82% 30%'}
      ];
      window.initialPriority = createBlankCreateFormData('todo').priority;
      window.priorities = buildPrioritySelectionPages().selectPriority.buildGroups()[0].items.map(i => i.priority.id);
      window.legacyPriority = selectDisplayData({
        state:createInitialKanbanIssuePanelFormState(), mode:'edit',
        createModeDefaults:createBlankCreateFormData('todo'),
        selectedIssue:{title:'legacy',description:null,status_id:'todo',priority:null},
        currentAssigneeIds:[],currentTagIds:[]
      }).priority;
      window.createdTags = 0;
      function App() {
        const [selected, setSelected] = useState(['system','bug']);
        return <>
          <IssueTagsRow availableTags={tags} selectedTagIds={selected} onTagsChange={setSelected}
            onCreateTag={() => 'created'}
            renderAddTagControl={({tags: choices,onTagToggle}) => <div data-testid="candidates">
              {choices.map(tag => <button key={tag.id} onClick={() => onTagToggle(tag.id)}>{'Add '+tag.name}</button>)}
            </div>} />
          <output>{selected.join(',')}</output>
          <SearchableTagDropdownContainer tags={tags.filter(tag => tag.id !== 'system')} selectedTagIds={selected}
            onTagToggle={id => setSelected([...selected,id])} onCreateTag={() => { window.createdTags++; return 'created'; }}
            trigger={<button>Open tag picker</button>} disabled={false} contentClassName="" />
        </>;
      }
      createRoot(document.getElementById('root')).render(<App />);
    `,
    loader: 'tsx',
    resolveDir: path.join(root, 'packages/web-core'),
  },
  bundle: true,
  write: false,
  platform: 'browser',
  format: 'iife',
  jsx: 'automatic',
  define: { 'process.env.NODE_ENV': '"production"' },
});
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.setContent('<html><body><div id="root"></div></body></html>');
  await page.addScriptTag({ content: outputFiles[0].text });
  await expect(
    page.getByRole('button', { name: 'system', exact: true })
  ).toBeDisabled();
  await expect(
    page.getByTestId('candidates').getByText('Add system')
  ).toHaveCount(0);
  assert.deepEqual(await page.evaluate(() => window.priorities), [
    'urgent',
    'high',
    'medium',
    'low',
  ]);
  assert.equal(await page.evaluate(() => window.initialPriority), 'medium');
  assert.equal(await page.evaluate(() => window.legacyPriority), 'medium');
  await page.getByRole('button', { name: 'bug', exact: true }).click();
  await expect(page.locator('output')).toHaveText('system');
  await page.getByRole('button', { name: 'Add feature', exact: true }).click();
  await expect(page.locator('output')).toHaveText('system,feature');
  await page.getByRole('button', { name: 'Open tag picker' }).click();
  const search = page.getByRole('textbox');
  await search.fill('system');
  await search.press('Enter');
  await search.press('Enter');
  assert.equal(await page.evaluate(() => window.createdTags), 0);
  await expect(page.locator('output')).toHaveText('system,feature');
  // Neither an existing system option nor a new reserved definition is offered.
  await expect(
    page
      .locator('[data-radix-popper-content-wrapper] button')
      .filter({ hasText: /^system$/ })
  ).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('html').evaluate((node) => node.classList.add('dark'));
  await expect(
    page.getByRole('button', { name: 'system', exact: true })
  ).toBeDisabled();
  assert.deepEqual(errors, []);
  console.log(
    'PASS: medium defaults, four priorities, system visibility/protection, normal tag edits, reserved picker, mobile/dark state'
  );
} finally {
  await browser.close();
}
