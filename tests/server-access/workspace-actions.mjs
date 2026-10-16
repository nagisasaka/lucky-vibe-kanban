// UI regression: shared access must not rewrite or depend on historical ownership.
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import assert from "node:assert/strict";
import http from "node:http";
import path from "node:path";
const result = await build({
  bundle: true,
  write: false,
  format: "esm",
  jsx: "automatic",
  stdin: {
    resolveDir: path.resolve("packages/local-web"),
    loader: "tsx",
    contents: `
    import React from 'react';
    import {createRoot} from 'react-dom/client';
    import i18n from 'i18next';
    import {initReactI18next} from 'react-i18next';
    import {IssueWorkspacesSection} from '../ui/src/components/IssueWorkspacesSection';
    await i18n.use(initReactI18next).init({lng:'en',resources:{en:{common:{}}}});
    const shared={id:'shared',localWorkspaceId:'shared',name:'Shared workspace',archived:false,filesChanged:0,linesAdded:0,linesRemoved:0,prs:[],owner:null,updatedAt:new Date().toISOString(),isOwnedByCurrentUser:false,canManageWorkspace:true};
    const cloud={...shared,id:'cloud',localWorkspaceId:'cloud',name:'Other cloud workspace',canManageWorkspace:undefined};
    window.opened=[];
    createRoot(document.getElementById('root')).render(<IssueWorkspacesSection workspaces={[shared,cloud]} onWorkspaceClick={id=>window.opened.push(id)}/>);
  `,
  },
});
const source = result.outputFiles[0].text;
const server = http.createServer((req, res) => {
  if (req.url === "/fixture.js") {
    res.setHeader("Content-Type", "text/javascript");
    res.end(source);
  } else {
    res.setHeader("Content-Type", "text/html");
    res.end(
      '<div id="root"></div><script type="module" src="/fixture.js"></script>',
    );
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
let browser;
try {
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.getByText("Shared workspace", { exact: true }).click();
  assert.deepEqual(await page.evaluate(() => window.opened), ["shared"]);
  await page.getByText("Other cloud workspace", { exact: true }).click();
  assert.deepEqual(await page.evaluate(() => window.opened), ["shared"]);
  assert.deepEqual(errors, []);
  console.log(
    "PASS: shared server Workspace opens without claiming historical ownership; cloud ownership restriction retained",
  );
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
}
