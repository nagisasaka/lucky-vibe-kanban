// Native nginx integration: no Docker, no real LVK database or agent calls.
import assert from "node:assert/strict";
import { spawn, spawnSync, execFileSync } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:http";
import https from "node:https";
import { isIP } from "node:net";
import { checkServerIdentity } from "node:tls";
import { X509Certificate } from "node:crypto";
import { mkdtemp, readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { test } from "node:test";
import {
  authority,
  renderNginx,
  serverSettings,
  withMcpIngress,
  validateSecrets,
} from "./server.mjs";

const independentHosts = [
  "dev.example.test",
  "alias.example.test",
  ["a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(61)].join("."),
];

const nginx = process.env.LVK_TEST_NGINX || "nginx";
const available = spawnSync(nginx, ["-v"]).status === 0;
if (!available && process.env.LVK_REQUIRE_NGINX_TEST === "1") {
  throw new Error("nginx is required for this CI gate");
}

function verifyIdentity(host, certificate) {
  // Node 22.23's checkServerIdentity passes IPv6 through domainToASCII, which
  // loses the IP literal. Verify IP SANs explicitly; TLS still checks the CA.
  if (isIP(host)) {
    return new X509Certificate(certificate.raw).checkIP(host)
      ? undefined
      : new Error(`TLS certificate does not cover IP ${host}`);
  }
  return checkServerIdentity(host, certificate);
}

for (const {
  host,
  prefix,
  managed = false,
  mcpBackend = 3000,
  independent = false,
} of [
  { host: "lvk.example.test", independent: true },
  { host: "127.0.0.1", managed: true, mcpBackend: 4021 },
  { host: "lvk.example.test" },
  { host: "127.0.0.1" },
  { host: "2001:db8::1" },
  { host: "lvk.example.test", prefix: "" },
  { host: "lvk.example.test", prefix: "/team/lvk", mcpBackend: 4021 },
])
  test(
    `HTTPS ingress protects HTTP/WS/SSE and isolates previews (${host}, ${independent ? "CSV routes" : managed ? "backend access" : `MCP ${prefix ?? "disabled"}`})`,
    {
      skip:
        !available && "nginx not installed (set LVK_TEST_NGINX to a binary)",
      timeout: 60000,
    },
    async (t) => {
      const dir = await mkdtemp(join(tmpdir(), "lvk-nginx-integration-"));
      t.after(() => rm(dir, { recursive: true, force: true }));
      const sockets = new Set();
      let streamEnded = false;
      const backend = createServer((req, res) => {
        if (independent)
          res.setHeader(
            "Set-Cookie",
            "app_session=response; Secure; HttpOnly; SameSite=Lax",
          );
        if (
          managed &&
          req.headers.host?.startsWith("5173.preview.example.test") &&
          req.headers.cookie !== "__Host-lvk_preview=fixture"
        ) {
          // This fixture represents preview admission, not project content.
          res.writeHead(401);
          res.end("preview admission required");
        } else if (req.url === "/mcp") {
          res.setHeader("Content-Type", "application/json");
          if (req.headers.authorization !== "Bearer fixture-token") {
            res.writeHead(401, {
              "WWW-Authenticate":
                'Bearer resource_metadata="https://lvk.example.test/.well-known/oauth-protected-resource/mcp"',
            });
            res.end("{}");
          } else {
            res.end(
              JSON.stringify({
                path: req.url,
                headers: req.headers,
                method: req.method,
              }),
            );
          }
        } else if (req.url.startsWith("/projects/")) {
          assert.equal(req.headers["accept-encoding"], undefined);
          res.setHeader("Content-Type", "text/html");
          res.end(
            "<!DOCTYPE html><html><head><title>Route fixture</title></head><body>Target</body></html>",
          );
        } else if (req.url === "/stream") {
          res.writeHead(200, { "Content-Type": "text/event-stream" });
          res.write("data: first\n\n");
          const timer = setTimeout(() => {
            streamEnded = true;
            res.end("data: done\n\n");
          }, 500);
          res.on("close", () => clearTimeout(timer));
        } else {
          res.setHeader("Content-Type", "application/json");
          res.end(JSON.stringify({ path: req.url, headers: req.headers }));
        }
      });
      backend.on("connection", (socket) => {
        sockets.add(socket);
        socket.on("close", () => sockets.delete(socket));
      });
      backend.on("upgrade", (req, socket) => {
        assert.equal(
          req.headers.authorization,
          independent ? "Bearer app-token" : undefined,
        );
        if (independent) {
          assert.equal(req.headers.host, "dev.example.test");
          assert.equal(req.headers.cookie, "app_session=fixture");
          assert.equal(req.headers.origin, "https://dev.example.test");
          assert.equal(req.headers["x-forwarded-proto"], "https");
        }
        assert.equal(req.headers["sec-websocket-protocol"], "vite-hmr");
        socket.write(
          "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Protocol: vite-hmr\r\n\r\n",
        );
      });
      backend.listen(0, "127.0.0.1");
      await once(backend, "listening");
      t.after(() => {
        for (const socket of sockets) socket.destroy();
        backend.close();
      });
      const backendPort = backend.address().port;
      const reservation = createServer();
      reservation.listen(0, "127.0.0.1");
      await once(reservation, "listening");
      const port = reservation.address().port;
      await new Promise((resolve) => reservation.close(resolve));
      for (const name of ["client", "proxy", "fastcgi", "uwsgi", "scgi"])
        await mkdir(join(dir, name));
      const certPath = join(dir, "fullchain.pem"),
        keyPath = join(dir, "privkey.pem");
      execFileSync(
        "openssl",
        [
          "req",
          "-x509",
          "-newkey",
          "rsa:2048",
          "-nodes",
          "-days",
          "1",
          "-subj",
          "/CN=lvk.example.test",
          "-addext",
          `subjectAltName=DNS:lvk.example.test,DNS:*.preview.example.test,${independentHosts.map((host) => `DNS:${host}`).join(",")},IP:127.0.0.1,IP:2001:db8::1`,
          "-keyout",
          keyPath,
          "-out",
          certPath,
        ],
        { stdio: "ignore" },
      );
      // Password lvk-test-only; this fixture is never used by the distribution.
      await writeFile(
        join(dir, "htpasswd"),
        "test:$2b$12$1ea3Q20isUydEeMVrwel0ecS0vCXLuctohZs1RomnFmxiGGxyCAOC\n",
      );
      await writeFile(join(dir, "mime.types"), "types { text/html html; }\n");
      let settings = serverSettings({
        LVK_HOST: host,
        ...(independent
          ? {
              LVK_PREVIEW_ROUTES: independentHosts
                .map((host) => `${host}:${backendPort}`)
                .join(","),
            }
          : { LVK_PREVIEW_DOMAIN: "preview.example.test" }),
        ...(managed
          ? { LVK_ACCESS_DEPLOYMENT: "public", LVK_PROXY_AUTH: "backend" }
          : {}),
      });
      if (prefix !== undefined) {
        settings = await withMcpIngress(
          settings,
          { LVK_MCP_CONFIG: "fixture.json" },
          async () =>
            JSON.stringify({ public_base_url: `https://${host}${prefix}` }),
        );
      }
      const template = await readFile(
        new URL("./nginx.conf.template", import.meta.url),
        "utf8",
      );
      const config = renderNginx(template, settings, { mcpBackend })
        // Node child pipes are Unix sockets, unlike Docker's stdout pipe; nginx
        // cannot reopen them as /dev/stdout. Keep this test's access log in its tempdir.
        .replace("/dev/stdout", join(dir, "access.log"))
        .replaceAll("/tmp/lvk-server", dir)
        .replace("/etc/nginx/mime.types", join(dir, "mime.types"))
        .replace("/run/lvk-secrets/tls/fullchain.pem", certPath)
        .replace("/run/lvk-secrets/tls/privkey.pem", keyPath)
        .replace("/run/lvk-secrets/htpasswd", join(dir, "htpasswd"))
        .replace("/run/lvk-maintenance/active", join(dir, "maintenance"))
        .replaceAll("listen 8443", `listen ${port}`)
        .replaceAll("127.0.0.1:3000", `127.0.0.1:${backendPort}`)
        .replaceAll("127.0.0.1:3001", `127.0.0.1:${backendPort}`)
        .replaceAll("127.0.0.1:4021", `127.0.0.1:${backendPort}`);
      const configPath = join(dir, "nginx.conf");
      await writeFile(configPath, config);
      const checked = spawnSync(
        nginx,
        ["-e", "stderr", "-t", "-c", configPath, "-p", dir],
        { encoding: "utf8" },
      );
      assert.equal(checked.status, 0, checked.stderr);
      const child = spawn(
        nginx,
        ["-e", "stderr", "-c", configPath, "-p", dir, "-g", "daemon off;"],
        { stdio: "pipe" },
      );
      let errors = "";
      child.stderr.on("data", (data) => {
        errors += data;
      });
      child.stdout.resume();
      t.after(async () => {
        if (child.exitCode === null) {
          const exited = once(child, "exit");
          child.kill("SIGTERM");
          const timer = setTimeout(() => child.kill("SIGKILL"), 1000);
          await exited;
          clearTimeout(timer);
        }
      });
      const ca = await readFile(certPath);
      const authorization = `Basic ${Buffer.from("test:lvk-test-only").toString("base64")}`;
      function request(
        path = "/",
        host = settings.app,
        headers = {},
        method = "GET",
      ) {
        return new Promise((resolve, reject) => {
          const req = https.request(
            {
              method,
              hostname: "127.0.0.1",
              port,
              path,
              servername: isIP(host) ? "" : host,
              checkServerIdentity: (_, certificate) =>
                verifyIdentity(host, certificate),
              ca,
              headers: { Host: authority(host), ...headers },
            },
            (res) => {
              let body = "";
              res.setEncoding("utf8");
              res.on("data", (chunk) => {
                body += chunk;
              });
              res.on("end", () =>
                resolve({ status: res.statusCode, body, headers: res.headers }),
              );
            },
          );
          req.on("upgrade", (res, socket) => {
            socket.destroy();
            resolve({ status: res.statusCode, headers: res.headers });
          });
          req.on("error", reject);
          // TLS and password hashing can exceed three seconds on a shared CI
          // host. This is a functional ingress test, not a latency benchmark.
          req.setTimeout(10000, () =>
            req.destroy(
              new Error(
                `fixture HTTPS timeout: ${method} ${path.split("?")[0]}`,
              ),
            ),
          );
          req.end();
        });
      }
      let ready = false;
      let connectionError;
      for (let i = 0; i < 50; i++) {
        try {
          await request();
          ready = true;
          break;
        } catch (error) {
          connectionError = error;
          await delay(20);
        }
      }
      assert(ready, `${errors}\n${connectionError?.message ?? ""}`);
      if (independent) {
        const key = await readFile(keyPath);
        const noBasic = {
          ...settings,
          managedAccess: true,
          legacyBasic: false,
        };
        validateSecrets(ca, key, null, noBasic);
        for (let i = 0; i < settings.routes.length; i++) {
          const routes = settings.routes.map((route, index) =>
            index === i ? { ...route, host: "missing.example.test" } : route,
          );
          assert.throws(
            () => validateSecrets(ca, key, null, { ...noBasic, routes }),
            /does not cover missing/,
          );
        }
        // Basic still protects LVK, but independent applications decide access.
        assert.equal((await request()).status, 401);
        for (const route of settings.routes) {
          assert.equal((await request("/", route.host)).status, 200);
          const response = await request("/path?query=kept", route.host, {
            Authorization: "Bearer app-token",
            Cookie: "app_session=fixture",
            Host: `${route.host}:443`,
            Origin: `https://${route.host}`,
            "X-Forwarded-Proto": "http",
            "X-Forwarded-For": "forged",
            "X-LVK-Internal-Token": "forged",
            "X-App-Auth": "custom-token",
          });
          assert.equal(response.status, 200);
          assert.deepEqual(response.headers["set-cookie"], [
            "app_session=response; Secure; HttpOnly; SameSite=Lax",
          ]);
          const received = JSON.parse(response.body);
          assert.equal(received.path, "/path?query=kept");
          assert.equal(received.headers.authorization, "Bearer app-token");
          assert.equal(received.headers.cookie, "app_session=fixture");
          assert.equal(received.headers.host, `${route.host}:443`);
          assert.equal(
            received.headers["x-forwarded-host"],
            `${route.host}:443`,
          );
          assert.equal(received.headers["x-forwarded-proto"], "https");
          assert.equal(received.headers["x-forwarded-port"], "443");
          assert.equal(received.headers["x-forwarded-for"], "127.0.0.1");
          assert.equal(received.headers["x-lvk-internal-token"], undefined);
          assert.equal(received.headers["x-app-auth"], "custom-token");
        }
        const ws = await request("/socket", "dev.example.test", {
          Authorization: "Bearer app-token",
          Cookie: "app_session=fixture",
          Origin: "https://dev.example.test",
          Upgrade: "websocket",
          Connection: "Upgrade",
          "Sec-WebSocket-Protocol": "vite-hmr",
          "Sec-WebSocket-Version": "13",
          "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
        });
        assert.equal(ws.status, 101);
        assert.equal(ws.headers["sec-websocket-protocol"], "vite-hmr");
        return;
      }
      if (managed) {
        // The backend owns authorization; nginx must forward the credentials
        // and cookies needed by it, while stripping local/relay capabilities.
        for (const path of [
          "/api/info",
          "/api/access/callback?code=fixture-only",
          "/.well-known/oauth-protected-resource/mcp",
        ]) {
          const response = await request(path, host, {
            Authorization: authorization,
            Cookie: "lvk_session=fixture-session",
            "X-LVK-Internal-Token": "forged-capability",
            "X-VK-Relayed": "1",
          });
          assert.equal(response.status, 200);
          const received = JSON.parse(response.body);
          assert.equal(received.headers.authorization, authorization);
          assert.equal(received.headers.cookie, "lvk_session=fixture-session");
          assert.equal(received.headers["x-lvk-internal-token"], undefined);
          assert.equal(received.headers["x-vk-relayed"], undefined);
        }
        assert.equal((await request("/mcp")).status, 401);
        assert.equal(
          (
            await request(
              "/mcp",
              host,
              { Authorization: "Bearer fixture-token" },
              "POST",
            )
          ).status,
          200,
        );
        assert.equal(
          (await request("/", "5173.preview.example.test")).status,
          401,
        );
        assert.equal(
          (
            await request("/", "5173.preview.example.test", {
              Cookie: "lvk_session=main-cookie",
            })
          ).status,
          401,
        );
        assert.equal(
          (
            await request("/", "5173.preview.example.test", {
              Cookie: "__Host-lvk_preview=fixture",
            })
          ).status,
          200,
        );
        return;
      }
      for (const path of ["/", "/api/info", "/api/events", "/stream"])
        assert.equal((await request(path)).status, 401, path);
      assert.equal(
        (
          await request("/", settings.app, {
            Authorization: "Basic d3Jvbmc6d3Jvbmc=",
          })
        ).status,
        401,
      );
      if (prefix !== undefined) {
        const endpoint = `${prefix}/mcp`;
        const anonymous = await request(endpoint);
        assert.equal(anonymous.status, 401);
        assert.match(anonymous.headers["www-authenticate"], /^Bearer /);
        assert.equal(
          (
            await request(endpoint, settings.app, {
              Authorization: authorization,
            })
          ).status,
          401,
        );
        const reply = await request(
          endpoint,
          settings.app,
          {
            Authorization: "Bearer fixture-token",
            Cookie: "private=fixture-cookie",
            "X-VK-Relayed": "1",
            "X-VK-Sig-Session": "forged",
            "Proxy-Authorization": "private",
            Origin: `https://${host}`,
          },
          "POST",
        );
        assert.equal(reply.status, 200);
        const received = JSON.parse(reply.body);
        assert.equal(received.path, "/mcp");
        assert.equal(received.method, "POST");
        assert.equal(received.headers.authorization, "Bearer fixture-token");
        assert.equal(received.headers.host, host);
        assert.equal(received.headers.origin, `https://${host}`);
        assert.equal(received.headers["x-forwarded-proto"], "https");
        for (const key of [
          "cookie",
          "proxy-authorization",
          "x-vk-relayed",
          "x-vk-sig-session",
        ])
          assert.equal(received.headers[key], undefined);
        const metadata = await request(
          `${prefix}/.well-known/oauth-protected-resource/mcp`,
          settings.app,
          { Authorization: "Bearer fixture-token", Cookie: "private=cookie" },
        );
        assert.equal(metadata.status, 200);
        const metadataRequest = JSON.parse(metadata.body);
        assert.equal(
          metadataRequest.path,
          "/.well-known/oauth-protected-resource/mcp",
        );
        assert.equal(metadataRequest.headers.authorization, undefined);
        assert.equal(metadataRequest.headers.cookie, undefined);
        for (const path of [
          `${endpoint}/`,
          `${endpoint}/other`,
          `${prefix}/api/info`,
          `${prefix}/.well-known/oauth-protected-resource/other`,
        ]) {
          const rejected = await request(path, settings.app, {
            Authorization: "Bearer fixture-token",
          });
          assert.equal(rejected.status, 401, path);
          assert.match(rejected.headers["www-authenticate"], /^Basic /);
        }
        assert.equal(
          (
            await request(endpoint, "5173.preview.example.test", {
              Authorization: "Bearer fixture-token",
            })
          ).status,
          401,
        );
        if (prefix) {
          const path = `${prefix}/projects/fixture/issues/issue/workspaces/workspace`;
          const locked = await request(path);
          assert.equal(locked.status, 401);
          assert.match(locked.headers["www-authenticate"], /^Basic /);
          const opened = await request(path, settings.app, {
            Authorization: authorization,
            "Accept-Encoding": "gzip",
          });
          assert.equal(opened.status, 200);
          assert(
            opened.body.includes(
              `<meta name="lvk-basepath" content="${prefix}">`,
            ),
          );
          assert.equal(opened.headers["x-frame-options"], "DENY");
          for (const blocked of [
            `${prefix}/api/preview/5173`,
            `${prefix}/api/host/fixture/preview/5173`,
          ])
            assert.equal(
              (
                await request(blocked, settings.app, {
                  Authorization: authorization,
                })
              ).status,
              403,
            );
        }
        const logs = await readFile(join(dir, "access.log"), "utf8");
        assert(!logs.includes("fixture-token"));
        assert(!logs.includes("fixture-cookie"));
      } else {
        for (const path of [
          "/mcp",
          "/.well-known/oauth-protected-resource/mcp",
        ]) {
          const rejected = await request(path, settings.app, {
            Authorization: "Bearer fixture-token",
          });
          assert.equal(rejected.status, 401);
          assert.match(rejected.headers["www-authenticate"], /^Basic /);
        }
      }
      const authorised = await request("/api/info?q=kept", settings.app, {
        Authorization: authorization,
        "X-VK-Relayed": "1",
      });
      assert.equal(authorised.status, 200);
      const data = JSON.parse(authorised.body);
      assert.equal(data.path, "/api/info?q=kept");
      assert.equal(data.headers.authorization, undefined);
      assert.equal(data.headers["x-vk-relayed"], undefined);
      assert.equal(data.headers["x-forwarded-proto"], "https");
      await writeFile(join(dir, "maintenance"), "");
      assert.equal(
        (
          await request("/api/info", settings.app, {
            Authorization: authorization,
          })
        ).status,
        503,
      );
      await rm(join(dir, "maintenance"));
      assert.equal(
        (
          await request("/api/info", settings.app, {
            Authorization: authorization,
          })
        ).status,
        200,
      );
      for (const path of [
        "/api/preview/5173",
        "/api/preview/5173/index.html",
        "/api/host/host-id/preview/5173",
      ])
        assert.equal(
          (await request(path, settings.app, { Authorization: authorization }))
            .status,
          403,
        );
      assert.equal(
        (await request("/", "5173.preview.example.test")).status,
        401,
      );
      assert.equal(
        (
          await request("/", "5173.preview.example.test", {
            Authorization: authorization,
          })
        ).status,
        200,
      );
      for (const target of [3000, 3001, 8443])
        assert.equal(
          (
            await request("/", `${target}.preview.example.test`, {
              Authorization: authorization,
            })
          ).status,
          403,
        );
      const wsHeaders = {
        Upgrade: "websocket",
        Connection: "Upgrade",
        "Sec-WebSocket-Protocol": "vite-hmr",
      };
      assert.equal((await request("/ws", settings.app, wsHeaders)).status, 401);
      assert.equal(
        (
          await request("/ws", settings.app, {
            ...wsHeaders,
            Authorization: authorization,
          })
        ).status,
        101,
      );
      assert.equal(
        (
          await request("/ws", "5173.preview.example.test", {
            ...wsHeaders,
            Authorization: authorization,
          })
        ).status,
        101,
      );
      await new Promise((resolve, reject) => {
        const req = https.get(
          {
            hostname: "127.0.0.1",
            port,
            path: "/stream",
            servername: isIP(settings.app) ? "" : settings.app,
            checkServerIdentity: (_, certificate) =>
              verifyIdentity(settings.app, certificate),
            ca,
            headers: {
              Host: authority(settings.app),
              Authorization: authorization,
            },
          },
          (res) => {
            res.once("data", (chunk) => {
              try {
                assert.match(chunk.toString(), /data: first/);
                assert.equal(streamEnded, false);
                resolve();
              } catch (error) {
                reject(error);
              } finally {
                req.destroy();
              }
            });
          },
        );
        req.on("error", reject);
        req.setTimeout(3000, () => req.destroy(new Error("stream timeout")));
      });
    },
  );
