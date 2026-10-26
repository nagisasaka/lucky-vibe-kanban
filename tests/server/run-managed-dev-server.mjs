// Isolated production UI manager + HTTP remote MCP + real stdio binary.
// Prerequisites: build vibe-kanban-mcp and agent-process-host with dev-cargo.
// LVK_DEV_SERVER_REAL_AGENT=1 also starts one real LVK Codex Session (status only).
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
await mkdir('.build', { recursive: true });
const instance = await mkdtemp(resolve('.build/devserver-fixture-'));
const env = {
  ...process.env,
  LVK_INSTANCE_DIR: instance,
  LVK_ACCESS_DEPLOYMENT: 'local',
  LVK_AUTH_MODE: 'local',
  LVK_PUBLIC_URL: '',
  HOST: '127.0.0.1',
  BACKEND_PORT: '0',
  PREVIEW_PROXY_PORT: '0',
  DISABLE_WORKTREE_CLEANUP: '1',
  LVK_TEST_MCP_BINARY: resolve('target/debug/vibe-kanban-mcp'),
  VIBE_KANBAN_AGENT_PROCESS_HOST: resolve('target/debug/agent-process-host'),
};
for (const key of ['LVK_MCP_CONFIG', 'VIBE_BACKEND_URL', 'VK_ALLOWED_ORIGINS']) delete env[key];
const child = spawn(process.execPath, ['scripts/dev-cargo.mjs', 'test', '-p', 'server', '--lib', 'isolated_dev_server_stdio_remote_and_ui', '--', '--ignored', '--nocapture'], { env, stdio: 'inherit' });
const forward = () => child.kill('SIGTERM');
process.on('SIGINT', forward);
process.on('SIGTERM', forward);
try {
  process.exitCode = await new Promise((done, reject) => { child.once('close', code => done(code ?? 1)); child.once('error', reject); });
} finally {
  process.off('SIGINT', forward);
  process.off('SIGTERM', forward);
  const remaining = async () => {
    const survivors = [];
    for (const pid of await readdir('/proc')) {
      if (!/^\d+$/.test(pid)) continue;
      try {
        const values = (await readFile(`/proc/${pid}/environ`, 'utf8')).split('\0');
        if (values.includes(`LVK_INSTANCE_DIR=${instance}`)) survivors.push(pid);
      } catch {}
    }
    return survivors;
  };
  let survivors = await remaining();
  for (let attempt = 0; survivors.length && attempt < 100; attempt++) {
    await delay(100);
    survivors = await remaining();
  }
  if (survivors.length) {
    process.exitCode = 1;
    console.error(`Retaining ${instance}: task processes remain: ${survivors.join(', ')}`);
  } else if (process.exitCode === 0) {
    try { await writeFile('.build/devserver-approval-reachability.json', await readFile(`${instance}/approval-reachability.json`)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    await rm(instance, { recursive: true });
    console.log('Fixture processes settled; isolated instance removed');
  } else {
    console.error(`Retaining failed fixture evidence: ${instance}`);
  }
}
