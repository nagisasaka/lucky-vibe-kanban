import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { fileURLToPath } from "node:url";

const result = await build({
  entryPoints: [
    fileURLToPath(new URL("./component.fixture.tsx", import.meta.url)),
  ],
  bundle: true,
  write: false,
  platform: "node",
  format: "cjs",
  jsx: "automatic",
  nodePaths: [
    fileURLToPath(
      new URL("../../packages/web-core/node_modules", import.meta.url),
    ),
  ],
});
const module = { exports: {} };
new Function("module", "exports", "require", result.outputFiles[0].text)(
  module,
  module.exports,
  (await import("node:module")).createRequire(import.meta.url),
);
const { render } = module.exports;
const revision = "0123456789abcdef0123456789abcdef01234567";

test("development: short SHA and full identity with dirty state", async () => {
  const html = await render({
    revision,
    kind: "development",
    release_tag: null,
    dirty: true,
  });
  assert.match(html, />0123456<\/p>/);
  assert.ok(html.includes(revision));
  assert.ok(html.includes("開発版"));
  assert.ok(html.includes("未コミット変更: あり"));
  assert.ok(html.includes("aria-label="));
});
test("release: existing full tag and clean state in details", async () => {
  const html = await render({
    revision,
    kind: "release",
    release_tag: "v0.1.50-lvk.1-server.9",
    dirty: false,
  });
  assert.ok(html.includes("リリース版 (v0.1.50-lvk.1-server.9)"));
  assert.ok(html.includes("未コミット変更: なし"));
});
test("partial identity does not guess kind or dirty; missing identity is hidden", async () => {
  const html = await render({
    revision,
    kind: null,
    release_tag: null,
    dirty: null,
  });
  assert.ok(html.includes("ビルド種別: 不明"));
  assert.ok(html.includes("未コミット変更: 不明"));
  for (const value of [
    undefined,
    null,
    { revision: "0.1.44" },
    { revision: "" },
  ]) {
    assert.equal(await render(value), "");
  }
});
test("English details and SHA-256 identifiers are supported", async () => {
  const html = await render(
    {
      revision: "a".repeat(64),
      kind: "development",
      release_tag: null,
      dirty: false,
    },
    "en",
  );
  assert.ok(html.includes("Development build"));
  assert.ok(html.includes("Uncommitted changes: no"));
  assert.match(html, />aaaaaaa<\/p>/);
});
