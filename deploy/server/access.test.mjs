import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  serverSettings,
  renderNginx,
  backendEnvironment,
  withMcpIngress,
} from "./server.mjs";
import { developmentEnvironment } from "./development.mjs";

const template = await readFile(
  new URL("./nginx.conf.template", import.meta.url),
  "utf8",
);
test("managed access forwards credentials to the backend and strips internal capabilities", async () => {
  const settings = serverSettings({
    LVK_HOST: "203.0.113.7",
    LVK_ACCESS_DEPLOYMENT: "public",
    LVK_PROXY_AUTH: "backend",
  });
  const nginx = renderNginx(template, settings);
  assert.match(nginx, /auth_basic off;/);
  assert.doesNotMatch(nginx, /auth_basic "LVK/);
  assert.match(nginx, /proxy_set_header Authorization \$http_authorization;/);
  assert.match(nginx, /proxy_set_header X-LVK-Internal-Token "";/);
  assert.match(nginx, /location = \/mcp/);
  assert.match(
    nginx,
    /location = \/\.well-known\/oauth-protected-resource\/mcp/,
  );
  assert.match(nginx, /location \/api\/access\//);
  assert.match(nginx, /location ~ \^\/api\/.*return 403/);
  await assert.rejects(
    withMcpIngress(settings, { LVK_MCP_CONFIG: "legacy.json" }),
    /migrate/,
  );
});
test("migration keeps the existing proxy guard until explicitly changed", () => {
  const settings = serverSettings({
    LVK_HOST: "203.0.113.7",
    LVK_ACCESS_DEPLOYMENT: "public",
  });
  assert.equal(settings.legacyBasic, true);
  assert.match(
    renderNginx(template, settings),
    /auth_basic "LVK private development server";/,
  );
  assert.equal(
    backendEnvironment({}, settings).LVK_PUBLIC_URL,
    "https://203.0.113.7",
  );
  const development = developmentEnvironment({}, settings);
  assert.equal(development.LVK_ACCESS_DEPLOYMENT, "public");
  assert.equal(development.LVK_PUBLIC_URL, "https://203.0.113.7:8443");
  const config = renderNginx(template, settings, { mcpBackend: 4021 });
  assert.match(
    config,
    /location \/api\/access\/ \{[\s\S]*?proxy_pass http:\/\/127\.0\.0\.1:4021;/,
  );
});
test("local and public compose boundaries and verified HTTPS probes", async () => {
  const local = await readFile(
    new URL("./compose.local.yaml", import.meta.url),
    "utf8",
  );
  const remote = await readFile(
    new URL("./compose.yaml", import.meta.url),
    "utf8",
  );
  assert.match(local, /127\.0\.0\.1:3000:3000/);
  assert.match(local, /LVK_ACCESS_DEPLOYMENT: local/);
  assert.match(remote, /LVK_ACCESS_DEPLOYMENT: public/);
  assert.doesNotMatch(remote, /3000:3000|4021:4021|docker\.sock/);
  for (const path of ["healthcheck.mjs", "development-healthcheck.mjs"]) {
    const source = await readFile(new URL(path, import.meta.url), "utf8");
    assert.doesNotMatch(source, /rejectUnauthorized: false/);
    assert.match(source, /rejectUnauthorized: true/);
  }
});

test("managed preview uses its own backend admission instead of LVK Basic credentials", () => {
  const settings = serverSettings({
    LVK_HOST: "203.0.113.7",
    LVK_PREVIEW_DOMAIN: "preview.example.test",
    LVK_ACCESS_DEPLOYMENT: "public",
    LVK_PROXY_AUTH: "backend",
  });
  const rendered = renderNginx(template, settings);
  assert.match(
    rendered,
    /auth_basic off;\s*location \/ \{ proxy_pass http:\/\/127\.0\.0\.1:3001; \}/,
  );
  assert.match(rendered, /location ~ \^\/api\/.*return 403/);
});
