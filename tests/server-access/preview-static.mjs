// Local fixture only: prove the public development preset's built frontend
// cannot expose files through Vite's development-only filesystem endpoints.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const root = process.cwd();
const web = path.join(root, "packages/local-web");
await readFile(path.join(web, "dist/index.html"));
await mkdir(path.join(root, ".build/tmp"), { recursive: true });
const fixture = await mkdtemp(path.join(root, ".build/tmp/access-preview-"));
const secretPath = path.join(fixture, "private-fixture.txt");
const sentinel = "LVK_PRIVATE_DEVELOPMENT_FIXTURE_NOT_BUSINESS_DATA";
await writeFile(secretPath, sentinel);
try {
  const reservation = net.createServer();
  reservation.listen(0, "127.0.0.1");
  await once(reservation, "listening");
  const port = reservation.address().port;
  await new Promise((resolve) => reservation.close(resolve));
  const child = spawn(
    process.execPath,
    [
      path.join(web, "node_modules/vite/bin/vite.js"),
      "preview",
      "--host",
      "127.0.0.1",
      "--port",
      String(port),
      "--strictPort",
    ],
    { cwd: web, stdio: ["ignore", "pipe", "pipe"] },
  );
  const exited = once(child, "exit");
  let logs = "";
  for (const stream of [child.stdout, child.stderr])
    stream.on("data", (data) => (logs = (logs + data).slice(-8192)));
  try {
    const origin = `http://127.0.0.1:${port}`;
    let ready = false;
    for (let attempt = 0; attempt < 300; attempt++) {
      if (child.exitCode !== null) throw new Error(logs);
      try {
        const response = await fetch(origin, {
          signal: AbortSignal.timeout(2000),
        });
        if (response.ok) {
          assert.match(await response.text(), /<html/);
          ready = true;
          break;
        }
      } catch {}
      await delay(100);
    }
    assert(ready, `Vite preview did not start: ${logs}`);
    for (const target of [
      `/@fs${secretPath}`,
      `/@fs/${secretPath}`,
      "/@vite/client",
    ]) {
      const response = await fetch(origin + target, {
        signal: AbortSignal.timeout(10000),
      });
      const text = await response.text();
      assert(
        !text.includes(sentinel),
        `private fixture leaked through ${target}`,
      );
      assert(
        !response.ok ||
          response.headers.get("content-type")?.includes("text/html"),
        `development endpoint unexpectedly active: ${target}`,
      );
    }
    console.log(
      "PASS: built Vite preview serves the app and no development filesystem/HMR endpoints",
    );
  } finally {
    child.kill("SIGTERM");
    await exited;
  }
} finally {
  await rm(fixture, { recursive: true, force: true });
}
