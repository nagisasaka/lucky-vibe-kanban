import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
await mkdir(".build", { recursive: true });
const instance = await mkdtemp(resolve(".build/reasoning-instance-"));
const log = createWriteStream(".build/reasoning-override-server.log");
const server = spawn(resolve("target/debug/server"), [], {
  env: {
    ...process.env,
    LVK_INSTANCE_DIR: instance,
    LVK_ACCESS_DEPLOYMENT: "local",
    HOST: "127.0.0.1",
    BACKEND_PORT: "0",
    PORT: "0",
    PREVIEW_PROXY_PORT: "0",
    DISABLE_WORKTREE_CLEANUP: "1",
    RUST_LOG: "info",
  },
  stdio: ["ignore", "pipe", "pipe"],
});
server.stdout.pipe(log);
server.stderr.pipe(log);
const stopped = new Promise((resolve) => server.once("close", resolve));
try {
  let baseURL;
  for (let n = 0; n < 120; n++) {
    if (server.exitCode !== null)
      throw Error("Server exited " + server.exitCode);
    try {
      const ports = JSON.parse(
        await readFile(instance + "/ports/vibe-kanban.port", "utf8"),
      );
      baseURL = "http://127.0.0.1:" + ports.main_port;
      if ((await fetch(baseURL + "/api/info")).ok) break;
    } catch {}
    baseURL = undefined;
    await delay(1000);
  }
  if (!baseURL) throw Error("Isolated server startup timed out");
  console.log("Isolated instance:", instance, "URL:", baseURL);
  const test = spawn(
    "pnpm",
    [
      "exec",
      "playwright",
      "test",
      "-c",
      "tests/server/playwright.config.ts",
      "tests/server/specs/reasoning-override.spec.ts",
      "tests/server/specs/application.spec.ts",
      "--reporter=list",
    ],
    {
      env: {
        ...process.env,
        LVK_E2E_BASE_URL: baseURL,
        LVK_E2E_REASONING: "1",
      },
      stdio: "inherit",
    },
  );
  const code = await new Promise((resolve) => test.once("close", resolve));
  process.exitCode = code ?? 1;
} finally {
  server.kill("SIGTERM");
  const timeout = setTimeout(() => server.kill("SIGKILL"), 15000);
  await stopped;
  clearTimeout(timeout);
  log.end();
  await rm(instance, { recursive: true, force: true });
  console.log("Isolated server stopped and task state removed");
}
