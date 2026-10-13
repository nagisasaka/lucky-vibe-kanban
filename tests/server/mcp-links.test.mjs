// Browser-level routing/auth-return fixture. No real LVK data, credentials or jobs.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:https";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { once } from "node:events";
import { chromium } from "@playwright/test";
import { createServer as createVite } from "vite";

const project = "11111111-1111-4111-8111-111111111111";
const issue = "22222222-2222-4222-8222-222222222222";
const workspace = "33333333-3333-4333-8333-333333333333";
const target = `/projects/${project}/issues/${issue}/workspaces/${workspace}`;

test(
  "actual LVK route IDs and navigation survive Basic authentication and a public prefix",
  { timeout: 180000 },
  async (t) => {
    const dir = await mkdtemp(join(tmpdir(), "lvk-mcp-link-browser-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const root = process.cwd();
    let vite;
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
        "/CN=127.0.0.1",
        "-addext",
        "subjectAltName=IP:127.0.0.1",
        "-keyout",
        join(dir, "key"),
        "-out",
        join(dir, "cert"),
      ],
      { stdio: "ignore" },
    );
    const accepted = `Basic ${Buffer.from("fixture:fixture-only").toString("base64")}`;
    let challenges = 0;
    const apiRequests = [];
    const sockets = new Set();
    const server = createServer(
      {
        key: await readFile(join(dir, "key")),
        cert: await readFile(join(dir, "cert")),
      },
      async (req, res) => {
        if (req.headers.authorization !== accepted) {
          challenges++;
          res.writeHead(401, {
            "WWW-Authenticate": 'Basic realm="Fixture LVK"',
          });
          res.end();
          return;
        }
        if (req.url.startsWith("/api")) {
          apiRequests.push(req.url);
          res.writeHead(503);
          res.end();
          return;
        }
        if (/^\/(?:team\/lvk\/)?projects\//.test(req.url)) {
          const prefix = req.url.startsWith("/team/lvk/") ? "/team/lvk" : "";
          const html = `<!doctype html><html><head>${prefix ? `<meta name="lvk-basepath" content="${prefix}">` : ""}<title>LVK router fixture</title></head><body><script type="module" src="/@fs/${root}/tests/server/fixtures/mcp-links.ts"></script></body></html>`;
          res.setHeader("Content-Type", "text/html");
          res.end(await vite.transformIndexHtml(req.url, html));
          return;
        }
        vite.middlewares(req, res, () => {
          res.writeHead(404);
          res.end();
        });
      },
    );
    server.on("connection", (socket) => {
      sockets.add(socket);
      socket.on("close", () => sockets.delete(socket));
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    t.after(async () => {
      for (const socket of sockets) socket.destroy();
      await new Promise((resolve) => server.close(resolve));
    });
    const base = `https://127.0.0.1:${server.address().port}`;
    vite = await createVite({
      configFile: resolve("packages/local-web/vite.config.ts"),
      root: resolve("packages/local-web"),
      cacheDir: join(dir, "vite-cache"),
      server: {
        middlewareMode: true,
        hmr: { server, protocol: "wss", clientPort: server.address().port },
        proxy: {},
        allowedHosts: ["127.0.0.1"],
      },
    });
    t.after(() => vite.close());

    const browser = await chromium.launch({ headless: true });
    t.after(() => browser.close());
    for (const prefix of ["", "/team/lvk"]) {
      const context = await browser.newContext({
        ignoreHTTPSErrors: true,
        httpCredentials: {
          username: "fixture",
          password: "fixture-only",
          origin: base,
        },
      });
      try {
        const page = await context.newPage();
        const errors = [];
        page.on("pageerror", (e) => errors.push(e.message));
        const response = await page.goto(base + prefix + target);
        assert.equal(response.status(), 200);
        await page.locator("#route-result").waitFor({ timeout: 120000 });
        const result = JSON.parse(
          await page.locator("#route-result").textContent(),
        );
        assert.deepEqual(result.destination, {
          kind: "project-issue-workspace",
          projectId: project,
          issueId: issue,
          workspaceId: workspace,
        });
        assert.equal(result.pathname, target);
        assert.equal(result.publicHref, prefix + target);
        assert(
          result.matches.some(
            (match) =>
              match.id ===
                "/_app/projects/$projectId_/issues/$issueId_/workspaces/$workspaceId" &&
              match.status === "success",
          ),
        );
        assert.equal(page.url(), base + prefix + target);
        await page.locator("#issue-link").click();
        await page.locator("#route-result").waitFor();
        const issueResult = JSON.parse(
          await page.locator("#route-result").textContent(),
        );
        assert.deepEqual(issueResult.destination, {
          kind: "project-issue",
          projectId: project,
          issueId: issue,
        });
        assert.equal(
          page.url(),
          `${base}${prefix}/projects/${project}/issues/${issue}`,
        );
        await page.reload();
        await page.locator("#route-result").waitFor();
        assert.deepEqual(
          JSON.parse(await page.locator("#route-result").textContent())
            .destination,
          issueResult.destination,
        );
        assert.deepEqual(errors, []);
      } finally {
        await context.close();
      }
    }
    assert(challenges >= 2, "Each new browser context must authenticate");
    assert.deepEqual(
      apiRequests,
      [],
      "Route matching must not start real API work",
    );
  },
);
