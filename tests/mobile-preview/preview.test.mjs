import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createServer, get } from "node:http";
import { spawnSync } from "node:child_process";
import { preview } from "vite";

const root = path.resolve(import.meta.dirname, "../..");

test("mobile preview serves SPA routes and preserves API HTTP/WS host and origin", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "lvk-preview-"));
  const backend = createServer((req, res) => {
    res.setHeader("Content-Type", "application/json");
    res.end(
      JSON.stringify({
        url: req.url,
        host: req.headers.host,
        origin: req.headers.origin,
      }),
    );
  });
  let upgrade;
  backend.on("upgrade", (req, socket) => {
    upgrade = {
      url: req.url,
      host: req.headers.host,
      origin: req.headers.origin,
    };
    // A rejection is sufficient to verify the proxy forwards upgrades unchanged.
    socket.end("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
  });
  await new Promise((resolve) => backend.listen(0, "127.0.0.1", resolve));
  const previousBackend = process.env.BACKEND_PORT;
  process.env.BACKEND_PORT = String(backend.address().port);
  let server;
  try {
    await writeFile(
      path.join(directory, "index.html"),
      '<div id="root">built preview</div>',
    );
    server = await preview({
      root: path.join(root, "packages/local-web"),
      configFile: path.join(root, "packages/local-web/vite.config.ts"),
      build: { outDir: directory },
      preview: { host: "127.0.0.1", port: 0, open: false },
    });
    const host = `127.0.0.1:${server.httpServer.address().port}`;
    const origin = `http://${host}`;
    assert.match(
      await (await fetch(`${origin}/workspaces/example`)).text(),
      /built preview/,
    );
    const response = await fetch(`${origin}/api/probe?value=1`, {
      headers: { Origin: origin },
    });
    assert.deepEqual(await response.json(), {
      url: "/api/probe?value=1",
      host,
      origin,
    });
    await new Promise((resolve, reject) => {
      const request = get(
        `${origin}/api/events/ws`,
        {
          headers: {
            Origin: origin,
            Connection: "Upgrade",
            Upgrade: "websocket",
          },
        },
        (response) => {
          response.resume();
          response.on("end", resolve);
        },
      );
      request.on("error", reject);
      request.setTimeout(5000, () =>
        request.destroy(new Error("Upgrade timed out")),
      );
    });
    assert.deepEqual(upgrade, { url: "/api/events/ws", host, origin });
  } finally {
    if (previousBackend === undefined) delete process.env.BACKEND_PORT;
    else process.env.BACKEND_PORT = previousBackend;
    if (server)
      await new Promise((resolve) => server.httpServer.close(resolve));
    backend.closeAllConnections();
    await new Promise((resolve) => backend.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});

test("preview launcher rebuilds before serving, stops on build failure and rejects invalid ports", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "lvk-preview-command-"));
  try {
    const log = path.join(directory, "calls");
    await mkdir(path.join(directory, "bin"));
    await writeFile(
      path.join(directory, "bin/pnpm"),
      '#!/bin/sh\nprintf "%s\\n" "$*" >> "$CALL_LOG"\nif [ "$4" = build ]; then exit "${BUILD_EXIT:-0}"; fi\n',
      { mode: 0o755 },
    );
    const env = {
      ...process.env,
      PATH: `${directory}/bin:${process.env.PATH}`,
      CALL_LOG: log,
      FRONTEND_PORT: "4030",
      BACKEND_PORT: "4021",
      HOST: "127.0.0.1",
    };
    const run = (overrides) =>
      spawnSync("bash", ["scripts/start-web-preview.sh"], {
        cwd: root,
        env: { ...env, ...overrides },
        encoding: "utf8",
      });
    assert.equal(run({}).status, 0);
    assert.deepEqual((await readFile(log, "utf8")).trim().split("\n"), [
      "--dir packages/local-web run build --outDir dist-preview",
      "--dir packages/local-web run preview --outDir dist-preview --host 127.0.0.1 --port 4030 --strictPort",
    ]);
    await writeFile(log, "");
    assert.equal(run({ BUILD_EXIT: "7" }).status, 7);
    assert.equal((await readFile(log, "utf8")).trim().split("\n").length, 1);
    await writeFile(path.join(directory, "bin/cargo"), "#!/bin/sh\nexit 0\n", {
      mode: 0o755,
    });
    const runContainer = (overrides) =>
      spawnSync("bash", ["scripts/start-container-dev.sh"], {
        cwd: root,
        env: {
          ...env,
          FRONTEND_PORT: "4020",
          PREVIEW_PROXY_PORT: "4022",
          ...overrides,
        },
        encoding: "utf8",
      });
    await writeFile(log, "");
    assert.equal(runContainer({ FRONTEND_MODE: "preview" }).status, 0);
    const previewCalls = (await readFile(log, "utf8")).trim().split("\n");
    assert.equal(previewCalls.length, 2);
    assert.match(previewCalls[0], /run build --outDir dist-preview/);
    assert.match(
      previewCalls[1],
      /exec concurrently .*backend:dev:watch.*run preview --outDir dist-preview/,
    );
    await writeFile(log, "");
    assert.equal(runContainer({ FRONTEND_MODE: "dev" }).status, 0);
    assert.match(await readFile(log, "utf8"), /run dev --host/);
    await writeFile(log, "");
    assert.equal(
      runContainer({ FRONTEND_MODE: "preview", BUILD_EXIT: "7" }).status,
      7,
    );
    assert.equal((await readFile(log, "utf8")).trim().split("\n").length, 1);
    assert.equal(runContainer({ FRONTEND_MODE: "invalid" }).status, 2);
    for (const overrides of [
      { FRONTEND_PORT: "bad" },
      { FRONTEND_PORT: "65536" },
      { BACKEND_PORT: "0" },
      { FRONTEND_PORT: "04021" },
    ]) {
      assert.equal(run(overrides).status, 2);
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
