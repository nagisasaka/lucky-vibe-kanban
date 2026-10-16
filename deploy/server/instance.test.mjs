import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { configuration, environment } from "../developer/instance.mjs";
import { renderNginx, serverSettings } from "./server.mjs";

test("instance environment isolates state and overrides stable server settings", () => {
  const inherited = {
    LVK_INSTANCES_ROOT: "/tmp/instances",
    LVK_HOST: "test.example",
    LVK_AUTH_MODE: "auth0",
    LVK_INSTANCE_DIR: "/stable",
    BACKEND_PORT: "3000",
    HOST: "0.0.0.0",
    PREVIEW_PROXY_PORT: "3001",
    VIBE_KANBAN_AGENT_PROCESS_HOST: "/usr/local/bin/agent-process-host",
  };
  const staging = configuration("staging", undefined, inherited);
  const env = environment(staging, "/builds/one", inherited);
  assert.equal(env.LVK_INSTANCE_DIR, "/tmp/instances/staging");
  assert.equal(env.BACKEND_PORT, "4020");
  assert.equal(env.LVK_ACCESS_DEPLOYMENT, "public");
  assert.equal(env.LVK_AUTH_MODE, "auth0");
  assert.equal(env.HOST, "127.0.0.1");
  assert.equal(env.VK_ALLOWED_ORIGINS, "https://test.example:8443");
  assert.equal(env.PREVIEW_PROXY_PORT, "0");
  assert.equal(
    env.VIBE_KANBAN_AGENT_PROCESS_HOST,
    "/builds/one/agent-process-host",
  );
  const card = environment(
    configuration("card-one", 4200, inherited),
    "/builds/two",
    inherited,
  );
  assert.notEqual(card.LVK_INSTANCE_DIR, env.LVK_INSTANCE_DIR);
  assert.equal(card.LVK_ACCESS_DEPLOYMENT, "local");
  assert.equal(card.LVK_AUTH_MODE, "");
  assert.notEqual(card.TMPDIR, env.TMPDIR);
  assert.equal(card.VK_ALLOWED_ORIGINS, "http://127.0.0.1:4200");
  for (const port of [0, 443, 3000, 3001, 4020, 8443, 8444, 65536, "bad"]) {
    assert.throws(() => configuration("card-one", port));
  }
  assert.throws(() => configuration("../stable", 4200));
  assert.throws(() => configuration("staging", 4200));
});

test("staging listener is opt-in and inherits authenticated ingress", async () => {
  const template = await readFile(
    new URL("./nginx.conf.template", import.meta.url),
    "utf8",
  );
  const ordinary = renderNginx(
    template,
    serverSettings({ LVK_HOST: "test.example" }),
  );
  assert(!ordinary.includes("listen 8444"));
  assert(!ordinary.includes("@@"));
  const enabled = renderNginx(
    template,
    serverSettings({ LVK_HOST: "test.example", LVK_ENABLE_STAGING: "1" }),
  );
  assert(enabled.includes("listen 8444 ssl"));
  assert(enabled.includes("proxy_pass http://127.0.0.1:4020"));
  assert(enabled.includes("proxy_pass http://127.0.0.1:3000"));
  assert(!enabled.includes("auth_basic off"));
  assert(enabled.includes('proxy_set_header Authorization ""'));
});

test(
  "foreground instance owns its lock and stops its child without affecting another instance",
  { timeout: 30000 },
  async () => {
    const { mkdtemp, mkdir, writeFile, chmod, rm } = await import(
      "node:fs/promises"
    );
    const { tmpdir } = await import("node:os");
    const { join } = await import("node:path");
    const { randomUUID } = await import("node:crypto");
    const { spawn } = await import("node:child_process");
    const { once } = await import("node:events");
    const { fileURLToPath } = await import("node:url");
    const root = await mkdtemp(join(tmpdir(), "lvk-instance-"));
    const processes = [];
    const id = randomUUID();
    async function fixture(name) {
      const build = join(root, name, "builds", id);
      await mkdir(build, { recursive: true });
      await writeFile(
        join(build, "build.json"),
        JSON.stringify({ id, revision: "test" }),
      );
      await writeFile(
        join(build, "server"),
        `#!/usr/bin/env node
console.log('READY ' + process.env.LVK_INSTANCE_DIR);
setInterval(() => {}, 1000);
process.on('SIGTERM', () => process.exit(0));
`,
      );
      await chmod(join(build, "server"), 0o700);
    }
    const runner = fileURLToPath(
      new URL("../developer/instance.mjs", import.meta.url),
    );
    function launch(name, port) {
      const child = spawn(
        process.execPath,
        [runner, "run", name, id, String(port)],
        {
          env: { ...process.env, LVK_INSTANCES_ROOT: root },
          stdio: ["ignore", "pipe", "pipe"],
        },
      );
      child.completion = once(child, "exit");
      child.output = "";
      child.stdout.on("data", (chunk) => {
        child.output += chunk;
      });
      processes.push(child);
      return child;
    }
    async function ready(child) {
      for (let i = 0; i < 200; i++) {
        if (child.output.includes("READY ")) return;
        if (child.exitCode !== null)
          throw new Error("Instance exited before ready");
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      throw new Error("Timed out waiting for instance");
    }
    try {
      await fixture("one");
      await fixture("two");
      const one = launch("one", 4200);
      await ready(one);
      const two = launch("two", 4201);
      await ready(two);
      const duplicate = launch("one", 4202);
      assert.notEqual((await duplicate.completion)[0], 0);
      one.kill("SIGTERM");
      assert.equal((await one.completion)[0], 0);
      assert.equal(two.exitCode, null);
      const restarted = launch("one", 4200);
      await ready(restarted);
      assert(restarted.output.includes(join(root, "one")));
    } finally {
      for (const child of processes)
        if (child.exitCode === null) child.kill("SIGTERM");
      await Promise.all(processes.map((child) => child.completion));
      await rm(root, { recursive: true, force: true });
    }
  },
);
