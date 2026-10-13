import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { serverSettings, withMcpIngress, renderNginx } from "./server.mjs";

test("MCP ingress is opt-in and validates its public path before changing authentication", async () => {
  const settings = serverSettings({ LVK_HOST: "lvk.example.test" });
  assert.equal(
    await withMcpIngress(settings, {}, () => {
      throw new Error("must not read");
    }),
    settings,
  );
  for (const public_base_url of [
    "http://lvk.example.test",
    "https://other.example",
    "https://lvk.example.test/",
    "https://lvk.example.test/a/../b",
    "https://lvk.example.test/a%2Fb",
    "https://lvk.example.test/a;b",
    'https://lvk.example.test/";auth_basic off;',
    "https://user:secret@lvk.example.test",
    "https://lvk.example.test?key=private",
    "https://lvk.example.test#private",
    "https://lvk.example.test//team",
  ]) {
    await assert.rejects(
      withMcpIngress(settings, { LVK_MCP_CONFIG: "test" }, async () =>
        JSON.stringify({ public_base_url }),
      ),
      /MCP ingress requires/,
    );
  }
  await assert.rejects(
    withMcpIngress(
      settings,
      { LVK_MCP_CONFIG: "test" },
      async () => "bad private JSON",
    ),
    /MCP ingress requires/,
  );
  const template = await readFile(
    new URL("./nginx.conf.template", import.meta.url),
    "utf8",
  );
  for (const prefix of ["", "/team/lvk"]) {
    const enabled = await withMcpIngress(
      { ...settings, staging: true },
      { LVK_MCP_CONFIG: "test" },
      async () =>
        JSON.stringify({
          public_base_url: `https://lvk.example.test${prefix}`,
        }),
    );
    const config = renderNginx(template, enabled);
    assert.equal(config.match(/auth_basic off/g).length, 2);
    assert(config.includes(`location = ${prefix}/mcp`));
    assert(!config.includes("@@"));
    assert(
      !config.slice(config.indexOf("listen 8444")).includes("auth_basic off"),
    );
  }
});
