// Public package upgrade and configuration conflicts. No global installation.
// OPENWIKI_PREVIOUS_BIN=/.../0.5.1/openwiki OPENWIKI_BIN=/.../0.7.2/openwiki
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";
const binary = process.env.OPENWIKI_BIN ?? "openwiki";
assert.ok(
  process.env.OPENWIKI_PREVIOUS_BIN,
  "Set OPENWIKI_PREVIOUS_BIN to the public 0.5.1 CLI",
);
const root = await mkdtemp(path.join(tmpdir(), "lvk-openwiki-host-"));
const memory = await mkdtemp(path.join(tmpdir(), "lvk-openwiki-memory-"));
const env = {
  ...process.env,
  OPENWIKI_TELEMETRY_DISABLED: "1",
  FORCE_COLOR: "0",
};
const run = (bin, args) =>
  spawnSync(bin, args, { env, encoding: "utf8", timeout: 300000 });
const install = (bin = binary, host = "codex") =>
  run(bin, ["integrations", "install", host, "--project", root]);
const ok = (r) => assert.equal(r.status, 0, r.stdout + r.stderr);
try {
  ok(run("git", ["init", root]));
  await mkdir(path.join(root, ".codex"));
  const config = path.join(root, ".codex/config.toml");
  const custom = 'model = "custom-model"\n';
  await writeFile(config, custom);
  ok(install(process.env.OPENWIKI_PREVIOUS_BIN));
  ok(install());
  ok(install());
  assert.ok((await readFile(config, "utf8")).startsWith(custom));
  ok(install(binary, "bob"));
  ok(run(binary, ["integrations", "uninstall", "bob", "--project", root]));
  ok(install());
  const intact = await readFile(config, "utf8");
  const displaced = intact.replace(
    "# OPENWIKI:MCP:END",
    '\n[projects."/fixture"]\ntrust_level = "trusted"\n# OPENWIKI:MCP:END',
  );
  await writeFile(config, displaced);
  assert.notEqual(
    install().status,
    0,
    "Upstream still refuses displaced marker",
  );
  assert.equal(await readFile(config, "utf8"), displaced);
  const adapted = run("node", [
    "scripts/dev-cargo.mjs",
    "run",
    "--quiet",
    "-p",
    "services",
    "--example",
    "openwiki_setup_fixture",
    "--",
    "install",
    root,
    memory,
    randomUUID(),
    "0".repeat(40),
  ]);
  ok(adapted);
  const repaired = await readFile(config, "utf8");
  assert.ok(repaired.includes('# OPENWIKI:MCP:END\n\n[projects."/fixture"]'));
  assert.ok(repaired.startsWith(custom));
  ok(install());
  for (const conflict of [
    repaired.replace('command = "openwiki"', 'command = "custom-openwiki"'),
    repaired.replace(
      'args = ["mcp", "--host", "codex"]',
      'args = ["mcp", "--host", "custom"]',
    ),
  ]) {
    await writeFile(config, conflict);
    assert.notEqual(install().status, 0);
    assert.equal(await readFile(config, "utf8"), conflict);
  }
  console.log(
    "PASS: public 0.5.1→0.7.2 upgrade/reinstall, shared host ownership, custom config conflict preservation, production marker repair.",
  );
} finally {
  await rm(root, { recursive: true, force: true });
  await rm(memory, { recursive: true, force: true });
}
