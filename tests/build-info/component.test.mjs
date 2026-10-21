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

test("known identity renders a native keyboard and touch popover trigger", async () => {
  const html = await render({
    revision,
    kind: "development",
    release_tag: null,
    dirty: true,
  });
  assert.match(html, /<button/);
  assert.match(html, /type="button"/);
  assert.match(html, />0123456<\/button>/);
  assert.match(html, /aria-haspopup="dialog"/);
  assert.match(html, /aria-expanded="false"/);
  assert.ok(html.includes('aria-label="ビルド情報: 0123456"'));
  assert.ok(!html.includes("title="), "details must not depend on hover");
});
test("partial identity still offers details, missing or invalid identity stays hidden", async () => {
  const html = await render({
    revision,
    kind: null,
    release_tag: null,
    dirty: null,
  });
  assert.match(html, /aria-haspopup="dialog"/);
  for (const value of [
    undefined,
    null,
    { revision: "0.1.44" },
    { revision: "" },
  ]) {
    assert.equal(await render(value), "");
  }
});
test("English trigger and SHA-256 identifiers are supported", async () => {
  const html = await render(
    {
      revision: "a".repeat(64),
      kind: "release",
      release_tag: "v0.1.50-lvk.1-server.9",
      dirty: false,
    },
    "en",
  );
  assert.ok(html.includes('aria-label="Build information: aaaaaaa"'));
  assert.match(html, />aaaaaaa<\/button>/);
});
