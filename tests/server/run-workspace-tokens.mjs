import { spawn } from "node:child_process";
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  readlink,
  rm,
} from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
await mkdir(".build", { recursive: true });
const instance = await mkdtemp(resolve(".build/tokens-instance-"));
const log = createWriteStream(".build/workspace-tokens-server.log");
const startServer = () =>
  spawn(resolve("target/debug/server"), [], {
    env: {
      ...process.env,
      LVK_INSTANCE_DIR: instance,
      LVK_ACCESS_DEPLOYMENT: "local",
      LVK_AUTH_MODE: "local",
      LVK_PUBLIC_URL: "",
      HOST: "127.0.0.1",
      BACKEND_PORT: "0",
      PORT: "0",
      PREVIEW_PROXY_PORT: "0",
      DISABLE_WORKTREE_CLEANUP: "1",
      RUST_LOG: "info",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
let server = startServer();
server.stdout.pipe(log, { end: false });
server.stderr.pipe(log, { end: false });
let spawnError;
server.on("error", (error) => {
  spawnError = error;
});
let stopped = new Promise((resolve) => server.once("close", resolve));
let testProcess;
let interrupted = false;
const interrupt = () => {
  interrupted = true;
  testProcess?.kill("SIGTERM");
  server.kill("SIGTERM");
};
process.on("SIGINT", interrupt);
process.on("SIGTERM", interrupt);
try {
  let baseURL;
  for (let n = 0; n < 120; n++) {
    if (spawnError) throw spawnError;
    if (interrupted) throw Error("Interrupted");
    if (server.exitCode !== null)
      throw Error("Server exited " + server.exitCode);
    try {
      const ports = JSON.parse(
        await readFile(instance + "/ports/vibe-kanban.port", "utf8"),
      );
      baseURL = "http://127.0.0.1:" + ports.main_port;
      if (
        (
          await fetch(baseURL + "/api/info", {
            signal: AbortSignal.timeout(3000),
          })
        ).ok
      )
        break;
    } catch {}
    baseURL = undefined;
    await delay(1000);
  }
  if (!baseURL) throw Error("Isolated server startup timed out");
  console.log("Isolated instance:", instance, "URL:", baseURL);
  testProcess = spawn(
    process.execPath,
    [
      resolve("node_modules/@playwright/test/cli.js"),
      "test",
      "-c",
      "tests/server/playwright.config.ts",
      "tests/server/specs/workspace-tokens.spec.ts",
      "--reporter=list",
    ],
    {
      env: {
        ...process.env,
        LVK_E2E_BASE_URL: baseURL,
        LVK_INSTANCE_DIR: instance,
        LVK_E2E_TOKENS: "1",
        LVK_E2E_TOKENS_RESTART: "0",
        LVK_E2E_CREDENTIALS_FILE: "",
        LVK_E2E_TRACE: "0",
      },
      stdio: "inherit",
    },
  );
  const code = await new Promise((resolve, reject) => {
    testProcess.once("close", resolve);
    testProcess.once("error", reject);
  });
  process.exitCode = code ?? 1;
  if (code === 0) {
    server.kill("SIGTERM");
    const restartStopTimeout = setTimeout(() => server.kill("SIGKILL"), 15000);
    await stopped;
    clearTimeout(restartStopTimeout);
    server = startServer();
    server.on("error", (error) => {
      spawnError = error;
    });
    stopped = new Promise((resolve) => server.once("close", resolve));
    server.stdout.pipe(log, { end: false });
    server.stderr.pipe(log, { end: false });
    let restartedURL;
    for (let n = 0; n < 120; n++) {
      if (spawnError) throw spawnError;
      if (interrupted || server.exitCode !== null)
        throw Error("Restart interrupted or exited");
      try {
        const ports = JSON.parse(
          await readFile(instance + "/ports/vibe-kanban.port", "utf8"),
        );
        const candidate = "http://127.0.0.1:" + ports.main_port;
        if (
          (
            await fetch(candidate + "/api/info", {
              signal: AbortSignal.timeout(1000),
            })
          ).ok
        ) {
          restartedURL = candidate;
          break;
        }
      } catch {}
      await delay(1000);
    }
    if (!restartedURL) throw Error("Isolated restart timed out");
    testProcess = spawn(
      process.execPath,
      [
        "node_modules/@playwright/test/cli.js",
        "test",
        "-c",
        "tests/server/playwright.config.ts",
        "tests/server/specs/workspace-tokens.spec.ts",
        "--reporter=list",
      ],
      {
        env: {
          ...process.env,
          LVK_E2E_BASE_URL: restartedURL,
          LVK_INSTANCE_DIR: instance,
          LVK_E2E_TOKENS: "1",
          LVK_E2E_TOKENS_RESTART: "1",
          LVK_E2E_CREDENTIALS_FILE: "",
          LVK_E2E_TRACE: "0",
        },
        stdio: "inherit",
      },
    );
    process.exitCode = await new Promise((resolve, reject) => {
      testProcess.once("close", resolve);
      testProcess.once("error", reject);
    });
  }
} finally {
  server.kill("SIGTERM");
  const timeout = setTimeout(() => server.kill("SIGKILL"), 15000);
  await stopped;
  clearTimeout(timeout);
  log.end();
  process.off("SIGINT", interrupt);
  process.off("SIGTERM", interrupt);
  // The unique instance environment is inherited by only our server/test
  // children, including Chromium. Never match a shared process by executable.
  const remaining = async () => {
    const owned = [];
    for (const entry of await readdir("/proc")) {
      if (!/^\d+$/.test(entry)) continue;
      try {
        const cwd = await readlink(`/proc/${entry}/cwd`);
        if (cwd !== process.cwd() && !cwd.startsWith(process.cwd() + "/"))
          continue;
        const env = await readFile(`/proc/${entry}/environ`, "utf8");
        if (env.split("\0").includes(`LVK_INSTANCE_DIR=${instance}`))
          owned.push(Number(entry));
      } catch {
        /* Process already exited or is not accessible. */
      }
    }
    return owned;
  };
  for (const pid of await remaining()) {
    try {
      process.kill(pid, "SIGTERM");
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  }
  let survivors = [];
  for (let n = 0; n < 50; n++) {
    survivors = await remaining();
    if (!survivors.length) break;
    await delay(100);
  }
  if (survivors.length)
    throw Error(
      `Task children still alive: ${survivors.join(", ")}; retaining ${instance}`,
    );
  await rm(instance, { recursive: true, force: true });
  console.log("Isolated server stopped and task state removed");
}
