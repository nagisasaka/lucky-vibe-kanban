// Real local server + SQLite + stdio MCP + Chromium. Never connects to a running
// LVK instance. Build server, agent-process-host, vibe-kanban-mcp and web first.
import { chromium, expect as baseExpect } from "@playwright/test";
const expect = baseExpect.configure({ timeout: 15000 });
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import path from "node:path";

const root = process.cwd();
const output = path.join(root, ".build/design-notes/e2e");
await mkdir(path.join(output, "screenshots"), { recursive: true });
const instance = await mkdtemp(path.join(output, "instance-"));
const env = {
  ...process.env,
  LVK_INSTANCE_DIR: instance,
  HOST: "127.0.0.1",
  BACKEND_PORT: "0",
  PREVIEW_PROXY_PORT: "0",
  LVK_ACCESS_DEPLOYMENT: "local",
  LVK_AUTH_MODE: "local",
  DISABLE_WORKTREE_CLEANUP: "1",
  RUST_LOG: "warn",
};
for (const key of [
  "LVK_PUBLIC_URL",
  "LVK_MCP_CONFIG",
  "VIBE_BACKEND_URL",
  "VK_ALLOWED_ORIGINS",
])
  delete env[key];
const children = [];
function start(binary, args = [], extra = {}) {
  const child = spawn(path.join(root, "target/debug", binary), args, {
    cwd: root,
    env: { ...env, ...extra },
    stdio: ["pipe", "pipe", "pipe"],
  });
  children.push(child);
  child.stderr.pipe(
    createWriteStream(path.join(output, `${binary}.stderr.log`)),
  );
  return child;
}
async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), 10000);
  await exited;
  clearTimeout(timer);
}
let browser;
let page;
let base;
async function api(url, method = "GET", body, expected = 200) {
  const response = await fetch(base + url, {
    method,
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
    redirect: "error",
  });
  const result = await response.json();
  assert.equal(
    response.status,
    expected,
    `${method} ${url}: ${JSON.stringify(result)}`,
  );
  if (expected === 200)
    assert.notEqual(result.success, false, JSON.stringify(result));
  return result;
}
try {
  const server = start("server");
  server.stdout.pipe(createWriteStream(path.join(output, "server.stdout.log")));
  for (let i = 0; i < 240; i++) {
    assert.equal(server.exitCode, null, "isolated server exited");
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, "server did not start");
  const info = (await api("/api/info")).data;
  const testedCommit = execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
  const dirtySource = execFileSync('git',['status','--porcelain'],{encoding:'utf8'}).trim().length > 0;
  assert.equal(info.build_info?.revision, testedCommit, 'server must be built from the tested commit');
  if (!dirtySource) assert.equal(info.build_info?.dirty, false, 'final server must be built from clean source');
  const config = info.config;
  await api("/api/config", "PUT", {
    ...config,
    language: "JA",
    theme: "DARK",
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    remote_onboarding_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ["workspaces-guide"] },
  });
  const project = (
    await api("/api/local/v1/projects", "POST", {
      organization_id: "00000000-0000-0000-0000-000000000002",
      name: "設計メモ検証",
      color: "#6366f1",
    })
  ).data.id;
  const mcp = start("vibe-kanban-mcp", ["--mode", "global"], {
    VIBE_BACKEND_URL: base,
  });
  let sequence = 0;
  const pending = new Map();
  createInterface({ input: mcp.stdout }).on("line", (line) => {
    const msg = JSON.parse(line);
    const entry = pending.get(msg.id);
    if (entry) {
      pending.delete(msg.id);
      clearTimeout(entry.timer);
      msg.error
        ? entry.reject(new Error(JSON.stringify(msg.error)))
        : entry.resolve(msg.result);
    }
  });
  const rpc = (method, params) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`MCP timeout: ${method}`));
      }, 30000);
      pending.set(id, { resolve, reject, timer });
      mcp.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
      );
    });
  await rpc("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "design-note-regression", version: "1" },
  });
  mcp.stdin.write(
    JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) +
      "\n",
  );
  const catalog = await rpc('tools/list', {});
  for (const name of ['list_design_notes','get_design_note','get_design_note_history','write_design_note']) assert.ok(catalog.tools.some(t => t.name === name));
  const tool = async (name, args, error = false) => {
    const result = await rpc('tools/call', {name, arguments: args});
    assert.equal(!!result.isError, error, JSON.stringify(result));
    return result.structuredContent ?? JSON.parse(result.content.find(c => c.type === 'text').text);
  };
  const notePath = `/api/projects/${project}/design-notes`;
  const csrfRejected = await fetch(base + notePath, {method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({request_id:randomUUID(),operation:{action:'create',title:'CSRF拒否',body:'must not persist'}}),redirect:'error'});
  assert.equal(csrfRejected.status,403);
  assert.equal((await api(notePath)).data.length,0);
  const write = async operation => (await api(notePath,'POST',{request_id:randomUUID(),operation})).data;
  browser = await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.LVK_CHROMIUM_EXECUTABLE ? {executablePath:process.env.LVK_CHROMIUM_EXECUTABLE} : {})});
  page = await browser.newPage({viewport:{width:1440,height:1000},locale:'ja-JP'});
  page.setDefaultTimeout(15000);
  const errors = [];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`${base}/projects/${project}`);
  await page.getByRole('button',{name:'設計メモ',exact:true}).click();
  const panel = page.getByTestId('design-notes');
  await panel.getByRole('button',{name:'新しい設計メモ'}).click();
  await panel.getByLabel('タイトル',{exact:true}).fill('最初の理想');
  await panel.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('人間の本文。目的を問い直す。');
  await panel.getByRole('button',{name:'保存',exact:true}).click();
  await expect(panel.getByTestId('note-body')).toContainText('人間の本文');
  let notes = (await api(notePath)).data;
  assert.equal(notes.length,1);
  const id = notes[0].id;
  assert.equal(notes[0].revision,1);
  const original = notes[0].body;
  await panel.getByRole('button',{name:'本文を編集'}).click();
  await panel.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('更新した人間の本文。');
  await panel.getByLabel('変更理由（任意）').fill('目的を明確化');
  await panel.getByRole('button',{name:'保存',exact:true}).click();
  await expect(panel.getByTestId('note-body')).toContainText('更新した人間の本文');
  assert.equal((await api(`${notePath}/${id}`)).data.note.revision,2);
  const addition = {heading:'会話での判断',content:'別ツールで目的を満たせるなら開発終了も合理的。',kind:'decision',source:'2026-10-21/message-1',speaker:'ユーザー',referenced_body_revision:1,short_quote:'目的を問い直す。'};
  const appendInput = {request_id:randomUUID(),operation:{action:'append',note_id:id,addition}};
  const appended = await tool('write_design_note',{project_id:project,input:appendInput});
  const replay = await tool('write_design_note',{project_id:project,input:appendInput});
  assert.equal(replay.addition.id,appended.addition.id);
  let detail = (await api(`${notePath}/${id}`)).data;
  assert.equal(detail.note.revision,2); assert.equal(detail.additions.length,1);
  await expect(panel.getByTestId('note-addition')).toContainText('会話での判断');
  await expect(panel.getByTestId('note-addition')).toContainText('2026-10-21/message-1');
  assert.notEqual(detail.additions[0].recorder,detail.additions[0].speaker);
  // UI/MCP concurrent edit: retain old input and reject it even after background refresh.
  await panel.getByRole('button',{name:'本文を編集'}).click();
  await panel.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('古い画面の入力は消さない');
  await tool('write_design_note',{project_id:project,input:{request_id:randomUUID(),operation:{action:'update',note_id:id,expected_revision:2,title:'最初の理想',body:'MCPから明示的に更新した本文'}}});
  await panel.getByRole('button',{name:'保存',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('入力は保持');
  await expect(panel.getByTestId('note-body-editor')).toContainText('古い画面の入力は消さない');
  assert.equal((await api(`${notePath}/${id}`)).data.note.body,'MCPから明示的に更新した本文');
  await panel.getByRole('button',{name:'キャンセル',exact:true}).click();
  // First version is reachable and restore appends a new version.
  await panel.getByRole('button',{name:'履歴・初版',exact:true}).click();
  await panel.getByRole('button',{name:/^初版 ·/}).click();
  await expect(panel.getByTestId('note-body')).toContainText('人間の本文。目的を問い直す。');
  await panel.getByRole('button',{name:'この版を新しい版として復元'}).click();
  await expect.poll(async()=>(await api(`${notePath}/${id}`)).data.note.revision).toBe(4);
  async function mcpSnapshot(args) {
    let content = '';
    for (;;) {
      const part = await tool('get_design_note',args);
      assert.ok(part.content_page.length <= 8000);
      content += part.content_page;
      if (!part.has_more) return JSON.parse(content);
      args = {...args, offset:part.next_offset, revision:part.revision};
    }
  }
  async function mcpHistory(args) {
    const result = [];
    for (;;) {
      const page = await tool('get_design_note_history',args);
      for (const item of page.items) result.push(await mcpSnapshot({...args,revision:item.revision}));
      if (!page.has_more) return result;
      args = {...args,after_revision:page.next_after_revision};
    }
  }
  const history = await mcpHistory({project_id:project,note_id:id});
  assert.equal(history.length,4); assert.equal(history[0].body,original); assert.equal(history[3].body,original);
  // Correction preserves original source/contents; hiding is reversible.
  let entry = panel.getByTestId('note-addition');
  await entry.getByRole('button',{name:'追記を訂正'}).click();
  await entry.getByLabel('出典（会話・発言の参照）').fill('2026-10-23/message-2');
  await entry.getByTestId('addition-editor').locator('[contenteditable=true]').fill('ユーザーが補足した判断。');
  await entry.getByRole('button',{name:'追記を保存'}).click();
  await expect(entry).toContainText('2026-10-23/message-2');
  await entry.getByRole('button',{name:'非表示にする'}).click();
  await expect(entry).toHaveCount(0);
  await panel.getByLabel('非表示の追記も表示').check();
  await expect(entry).toContainText('（非表示）');
  await entry.getByRole('button',{name:'追記の履歴'}).click();
  await expect(entry).toContainText('2026-10-21/message-1');
  await expect(entry).toContainText('別ツールで目的を満たせるなら');
  await entry.getByRole('button',{name:'追記の履歴'}).click();
  await entry.getByRole('button',{name:'再表示',exact:true}).click();
  page.once('dialog',d=>d.accept());
  await entry.getByRole('button',{name:'本文へ統合'}).click();
  await expect(panel.getByTestId('note-body')).toContainText('ユーザーが補足した判断');
  detail = (await api(`${notePath}/${id}`)).data;
  assert.equal(detail.note.revision,5); assert.equal(detail.additions.length,1);
  assert.equal(detail.additions[0].revision,4);
  const additionHistory = await mcpHistory({project_id:project,note_id:id,addition_id:appended.addition.id});
  assert.equal(additionHistory.length,4); assert.equal(additionHistory[0].source,addition.source);
  // Two real browser tabs: the second editor cannot overwrite a committed first edit.
  const second = await browser.newPage({viewport:{width:1280,height:900}});
  await second.goto(`${base}/projects/${project}`);
  await second.getByRole('button',{name:'設計メモ',exact:true}).click();
  await second.getByTestId('design-notes').getByRole('button',{name:/最初の理想/}).click();
  await second.getByRole('button',{name:'本文を編集'}).click();
  await second.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('二つ目の画面の未保存本文');
  await panel.getByRole('button',{name:'本文を編集'}).click();
  await panel.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('一つ目の画面で保存した本文');
  await panel.getByRole('button',{name:'保存',exact:true}).click();
  await expect(panel.getByTestId('note-body')).toContainText('一つ目の画面で保存した本文');
  await second.getByRole('button',{name:'保存',exact:true}).click();
  await expect(second.getByRole('alert')).toContainText('入力は保持');
  await expect(second.getByTestId('note-body-editor')).toContainText('二つ目の画面の未保存本文');
  await expect(second.getByTestId('body-conflict-current')).toContainText('一つ目の画面で保存した本文');
  await second.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('一つ目の画面で保存した本文');
  await second.getByRole('button',{name:'現在版を確認したので本文の下書きを再適用'}).click();
  await second.getByRole('button',{name:'保存',exact:true}).click();
  await expect(second.getByTestId('note-body')).toContainText('一つ目の画面で保存した本文');
  await second.close();
  // Addition conflict also keeps the draft and requires explicit reapplication.
  entry = panel.getByTestId('note-addition');
  await entry.getByRole('button',{name:'追記を訂正'}).click();
  await expect(entry.getByRole('button',{name:'追記を訂正'})).toBeDisabled();
  await entry.getByTestId('addition-editor').locator('[contenteditable=true]').fill('保持した追記の下書き');
  const beforeCorrection = (await api(`${notePath}/${id}`)).data.additions[0];
  await tool('write_design_note',{project_id:project,input:{request_id:randomUUID(),operation:{action:'correct_addition',note_id:id,addition_id:beforeCorrection.id,expected_revision:beforeCorrection.revision,addition:{...addition,source:'message/concurrent',content:'別の画面からの訂正'}}}});
  await entry.getByRole('button',{name:'追記を保存'}).click();
  await expect(panel.getByRole('alert')).toContainText('入力は保持');
  await expect(entry.getByTestId('addition-editor')).toContainText('保持した追記の下書き');
  await expect(entry).toContainText('message/concurrent');
  await entry.getByRole('button',{name:'現在版を確認したので追記の下書きを再適用'}).click();
  await entry.getByRole('button',{name:'追記を保存'}).click();
  await expect(entry.getByTestId('addition-editor')).toHaveCount(0);
  await expect(entry).toContainText('保持した追記の下書き');
  // Remote hiding while editing must not unmount and discard an unsaved draft.
  await panel.getByLabel('非表示の追記も表示').uncheck();
  await entry.getByRole('button',{name:'追記を訂正'}).click();
  await expect(entry.getByRole('button',{name:'非表示にする'})).toBeDisabled();
  await entry.getByTestId('addition-editor').locator('[contenteditable=true]').fill('非表示になっても保持する下書き');
  const beforeHide = (await api(`${notePath}/${id}`)).data.additions[0];
  await tool('write_design_note',{project_id:project,input:{request_id:randomUUID(),operation:{action:'set_addition_hidden',note_id:id,addition_id:beforeHide.id,expected_revision:beforeHide.revision,hidden:true}}});
  await expect(entry).toContainText('（非表示）');
  await expect(entry.getByTestId('addition-editor')).toContainText('非表示になっても保持する下書き');
  await entry.getByRole('button',{name:'追記を保存'}).click();
  await expect(panel.getByRole('alert')).toContainText('入力は保持');
  await entry.getByRole('button',{name:'現在版を確認したので追記の下書きを再適用'}).click();
  await entry.getByRole('button',{name:'追記を保存'}).click();
  await expect(entry).toHaveCount(0);
  await panel.getByLabel('非表示の追記も表示').check();
  await expect(entry).toContainText('非表示になっても保持する下書き');
  await entry.getByRole('button',{name:'再表示',exact:true}).click();
  // API idempotency after intervening updates and scope mismatches.
  const current = (await api(`${notePath}/${id}`)).data.note;
  const updateInput = {request_id:randomUUID(),operation:{action:'update',note_id:id,expected_revision:current.revision,title:current.title,body:current.body}};
  const once = (await api(notePath,'POST',updateInput)).data;
  assert.equal((await api(notePath,'POST',updateInput)).data.note.revision,once.note.revision);
  await api(notePath,'POST',{...updateInput,operation:{...updateInput.operation,body:'same key changed payload'}},409);
  const foreign = (await api('/api/local/v1/projects','POST',{organization_id:'00000000-0000-0000-0000-000000000002',name:'別Project',color:'#6366f1'})).data.id;
  await api(`/api/projects/${foreign}/design-notes/${id}`,'GET',undefined,404);
  await api(`/api/projects/${foreign}/design-notes`,'POST',{request_id:randomUUID(),operation:{action:'restore',note_id:id,expected_revision:once.note.revision,revision:1}},404);
  await tool('get_design_note',{project_id:foreign,note_id:id},true);
  const beforeDelete = (await api(`${notePath}/${id}`)).data;
  const protectedDelete = await api(`/api/local/v1/projects/${project}`,'DELETE',undefined,409);
  assert.match(JSON.stringify(protectedDelete),/履歴/);
  assert.match(JSON.stringify(protectedDelete),/一時的/);
  assert.deepEqual((await api(`${notePath}/${id}`)).data,beforeDelete);

  // Multiple notes, visible proposal/open question distinction, safe Markdown renderer.
  await write({action:'append',note_id:id,addition:{...addition,heading:'AI提案',kind:'proposal',speaker:'AI',content:'<script>window.__noteXss=true</script>\n[unsafe](javascript:alert(1))'}});
  await write({action:'append',note_id:id,addition:{...addition,heading:'未決事項',kind:'open_question',speaker:'ユーザー',content:'保持期限は未合意'}});
  await panel.getByRole('button',{name:'新しい設計メモ'}).click();
  await panel.getByLabel('タイトル',{exact:true}).fill('別の設計メモ');
  await panel.getByTestId('note-body-editor').locator('[contenteditable=true]').fill('別の本文');
  await panel.getByRole('button',{name:'保存',exact:true}).click();
  await expect(panel.getByTestId('note-body')).toContainText('別の本文');
  assert.equal((await tool('list_design_notes',{project_id:project})).length,2);
  await panel.getByRole('button',{name:/最初の理想/}).click();
  await expect(panel.getByTestId('note-addition').filter({hasText:'AI提案'})).toContainText('提案');
  await expect(panel.getByTestId('note-addition').filter({hasText:'保持期限は未合意'})).toContainText('未決事項');
  assert.equal(await page.evaluate(()=>window.__noteXss),undefined);
  assert.equal(await panel.locator('a[href^="javascript:"]').count(),0);
  const listBounds = await panel.locator('aside').boundingBox();
  const mainBounds = await panel.locator('main').boundingBox();
  assert.ok(listBounds.x + listBounds.width <= mainBounds.x + 1, 'desktop list and detail must be side by side');
  await panel.locator('main').evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({path:path.join(output,'screenshots/desktop-dark.png')});
  // Narrow list -> detail navigation, no horizontal page overflow, both themes.
  for (const width of [390,320]) {
    await page.setViewportSize({width,height:844});
    // ProjectKanban remounts its layout at the mobile breakpoint. Start at the
    // route's list, as a mobile user does, then verify list -> detail -> list.
    await page.goto(`${base}/projects/${project}`);
    await page.getByRole('button',{name:'設計メモ',exact:true}).click();
    await panel.getByRole('button',{name:/最初の理想/}).click();
    await expect(panel.getByTestId('note-body')).toBeVisible();
    await panel.getByRole('button',{name:'一覧へ',exact:true}).click();
    await expect(panel.getByRole('button',{name:'新しい設計メモ'})).toBeVisible();
    await panel.getByRole('button',{name:/最初の理想/}).click();
    await expect(panel.getByTestId('note-body')).toBeVisible();
    assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`overflow at ${width}`);
  }
  await api('/api/config','PUT',{...(await api('/api/info')).data.config,theme:'LIGHT'});
  await page.reload();
  await page.getByRole('button',{name:'設計メモ',exact:true}).click();
  await panel.getByRole('button',{name:/最初の理想/}).click();
  await expect(page.locator('html')).toHaveClass(/light/);
  await expect(panel.getByTestId('note-body')).toContainText('一つ目の画面で保存した本文');
  await page.screenshot({path:path.join(output,'screenshots/mobile-light.png')});
  await page.reload();
  await page.getByRole('button',{name:'設計メモ',exact:true}).click();
  await page.getByTestId('design-notes').getByRole('button',{name:/最初の理想/}).click();
  await expect(page.getByTestId('note-body')).toContainText('一つ目の画面で保存した本文');
  assert.deepEqual(errors,[]);
  // Real stdio transport: maximum body + addition must never escape in a write receipt.
  const longText = '😀'.repeat(100000);
  const longCreate = {request_id:randomUUID(),operation:{action:'create',title:'bounded receipt',body:longText}};
  const createdReceipt = await tool('write_design_note',{project_id:foreign,input:longCreate});
  const longId = createdReceipt.note.id;
  const longAppend = {request_id:randomUUID(),operation:{action:'append',note_id:longId,addition:{heading:'long',content:longText,kind:'proposal',source:'synthetic/test',speaker:'AI',referenced_body_revision:1}}};
  const addedReceipt = await tool('write_design_note',{project_id:foreign,input:longAppend});
  await tool('write_design_note',{project_id:foreign,input:{request_id:randomUUID(),operation:{action:'update',note_id:longId,expected_revision:1,title:'changed',body:'changed'}}});
  for (const [input,receipt] of [[longCreate,createdReceipt],[longAppend,addedReceipt]]) {
    const retry = await tool('write_design_note',{project_id:foreign,input});
    assert.deepEqual(retry,receipt);
    assert.ok(Buffer.byteLength(JSON.stringify(receipt)) < 256);
    assert.equal(receipt.note.revision,1);
    assert.deepEqual(Object.keys(receipt.note).sort(),['id','revision']);
    if (receipt.addition) assert.deepEqual(Object.keys(receipt.addition).sort(),['id','revision']);
  }
  assert.equal((await mcpSnapshot({project_id:foreign,note_id:longId,revision:1})).body,longText);
  assert.equal((await mcpSnapshot({project_id:foreign,note_id:longId,addition_id:addedReceipt.addition.id,revision:1})).content,longText);
  console.log(JSON.stringify({result:'passed',commit:testedCommit,dirty:dirtySource,serverBuild:info.build_info,chromium:browser.version(),checks:['UI create/edit/multiple notes','MCP append and identical retry','UI/MCP conflict','UI/UI conflict','initial/history/restore','correction/hiding/attribution history','explicit integration','API retry and project boundary','MCP project boundary','safe Markdown','desktop/mobile 390/320','reload'],ports:new URL(base).port},null,2));
} catch(error) {
  if(page) await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
  throw error;
} finally {
  await browser?.close();
  for(const child of children.reverse()) await stop(child);
  await rm(instance,{recursive:true,force:true});
}
