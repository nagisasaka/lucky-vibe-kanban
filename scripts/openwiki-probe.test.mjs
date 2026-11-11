import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

test("version probe requires the public server identity and exact pin", () => {
  const root = mkdtempSync(path.join(tmpdir(), "lvk-openwiki-probe-"));
  try {
    const binary = path.join(root, "fake-mcp");
    for (const [info, success] of [
      [{ name: "openwiki", version: "0.9.0" }, true],
      [{ name: "openwiki", version: "0.7.2" }, false],
      [{ name: "other", version: "0.9.0" }, false],
      [{ name: "openwiki" }, false],
    ]) {
      writeFileSync(
        binary,
        `#!/usr/bin/env node
process.stdin.once('data', bytes => {
  const r = JSON.parse(bytes);
  if (r.method !== 'initialize' || process.argv.slice(2).join(' ') !== 'mcp --host codex') process.exit(1);
  console.log(JSON.stringify({jsonrpc:'2.0', id:r.id, result:{serverInfo:${JSON.stringify(info)}}}));
});
`,
        { mode: 0o755 },
      );
      const result = spawnSync(
        "node",
        [
          new URL("../assets/openwiki-probe.mjs", import.meta.url).pathname,
          binary,
          "0.9.0",
        ],
        { encoding: "utf8", timeout: 5000 },
      );
      assert.equal(result.status, success ? 0 : 1, result.stderr);
      assert.equal(result.stdout.trim(), success ? "0.9.0" : "");
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
