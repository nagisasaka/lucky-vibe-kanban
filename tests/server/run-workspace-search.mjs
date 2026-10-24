// Creates and removes only this invocation's isolated instance. Never accepts a
// caller-supplied DB, runtime directory or server URL.
import { isolatedEnvironment } from "./fixtures/issue-comments-env.mjs";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
await mkdir(".build", { recursive: true });
const instance = await mkdtemp(resolve(".build/search-instance-"));
await writeFile(
  `${instance}/search-fixture-owner`,
  "LOCAL-162 isolated fixture\n",
);
// Search tests do not use agent providers. Do not launch a host-installed
// provider for background UI capability discovery inside these worktrees.
const fixtureBin = `${instance}/fixture-bin`;
await mkdir(fixtureBin);
for (const command of ["codex", "claude"]) {
  await writeFile(
    `${fixtureBin}/${command}`,
    "#!/bin/sh\n# LOCAL-162 fixture only\nexit 1\n",
    { mode: 0o755 },
  );
}
const env = {
  ...isolatedEnvironment(process.env, instance, process.cwd()),
  CARGO_BUILD_JOBS: "1",
};
env.PATH = `${fixtureBin}:${env.PATH ?? "/usr/bin:/bin"}`;

const log = createWriteStream(".build/workspace-search-server.log");
const server = spawn(resolve("target/debug/server"), [], {
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
server.stdout.pipe(log, { end: false });
server.stderr.pipe(log, { end: false });
let spawnError;
server.on("error", (error) => {
  spawnError = error;
});
const stopped = new Promise((done) => server.once("close", done));
let child;
let childStopped;
let interrupted = false;
const interrupt = () => {
  interrupted = true;
  child?.kill("SIGTERM");
  server.kill("SIGTERM");
};
process.on("SIGINT", interrupt);
process.on("SIGTERM", interrupt);
try {
  let base;
  for (let n = 0; n < 120; n++) {
    if (spawnError) throw spawnError;
    if (interrupted || server.exitCode !== null)
      throw Error("Fixture server stopped");
    try {
      const ports = JSON.parse(
        await readFile(`${instance}/ports/vibe-kanban.port`, "utf8"),
      );
      const url = `http://127.0.0.1:${ports.main_port}`;
      if (
        (await fetch(`${url}/api/info`, { signal: AbortSignal.timeout(1000) }))
          .ok
      ) {
        base = url;
        break;
      }
    } catch {}
    await delay(500);
  }
  if (!base) throw Error("Isolated server startup timed out");
  console.log("Search fixture:", instance, base);
  child = spawn(
    process.execPath,
    [
      resolve("node_modules/@playwright/test/cli.js"),
      "test",
      "-c",
      "tests/server/playwright.config.ts",
      "workspace-search.spec.ts",
      "--reporter=list",
    ],
    {
      env: { ...env, LVK_E2E_BASE_URL: base, LVK_E2E_SEARCH: "1" },
      stdio: "inherit",
    },
  );
  childStopped = new Promise((done, reject) => {
    child.once("close", done);
    child.once("error", reject);
  });
  process.exitCode = (await childStopped) ?? 1;
} finally {
  child?.kill("SIGTERM");
  if (childStopped) await childStopped.catch(() => {});
  server.kill("SIGTERM");
  const timeout = setTimeout(() => server.kill("SIGKILL"), 15000);
  await stopped;
  clearTimeout(timeout);
  log.end();
  await rm(instance, { recursive: true, force: true });
  process.off("SIGINT", interrupt);
  process.off("SIGTERM", interrupt);
}
