#!/usr/bin/env node
/** Workspace-owned fixture preview. Start only through the managed DevServer. */
import { randomBytes, timingSafeEqual } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import net from "node:net";

const repo = fileURLToPath(new URL("..", import.meta.url));
const directory = path.join(repo, ".build/settings-preview");
const credentialsPath = path.join(directory, "credentials.json");
const port = 18444;
const command = process.argv[2];
if (!["--prepare", "--run", "--check"].includes(command))
  throw Error(
    "Usage: node scripts/settings-preview.mjs --prepare|--run|--check",
  );
await mkdir(directory, { recursive: true, mode: 0o700 });
if (command === "--prepare") {
  try {
    await readFile(credentialsPath);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    await writeFile(
      credentialsPath,
      JSON.stringify({
        username: "settings-demo",
        password: randomBytes(24).toString("base64url"),
      }) + "\n",
      { mode: 0o600, flag: "wx" },
    );
  }
  console.log(
    `Prepared isolated browser fixture. Credential file: ${credentialsPath}`,
  );
  console.log(
    "Managed DevServer command: node scripts/settings-preview.mjs --run",
  );
  console.log(
    "Requested existing route: https://preview2.luckyvibekanban.com → 127.0.0.1:18444",
  );
  console.log(
    "No server started. The operator must confirm the public slot is assigned before starting.",
  );
} else {
  const { username, password } = JSON.parse(
    await readFile(credentialsPath, "utf8"),
  );
  if (!username || !password)
    throw Error("Missing workspace fixture credentials");
  // Refuse an occupied port. Never adopt or stop an existing listener.
  if (command === "--run")
    await new Promise((resolve, reject) => {
      const probe = net.createServer();
      probe.once("error", reject);
      probe.listen(port, "127.0.0.1", () => probe.close(resolve));
    });
  const { createServer, mergeConfig, loadConfigFromFile } = await import(
    "vite"
  );
  const loaded = await loadConfigFromFile(
    { command: "serve", mode: "development" },
    path.join(repo, "tests/settings-redesign/vite.config.ts"),
  );
  if (!loaded) throw Error("Fixture Vite configuration unavailable");
  const config = {
    ...loaded.config,
    server: { ...loaded.config.server, fs: undefined },
  };
  const expected = Buffer.from(
    "Basic " + Buffer.from(username + ":" + password).toString("base64"),
  );
  const protect = (request, response, next) => {
    const supplied = Buffer.from(request.headers.authorization ?? "");
    if (
      supplied.length !== expected.length ||
      !timingSafeEqual(supplied, expected)
    ) {
      response.writeHead(401, {
        "WWW-Authenticate":
          'Basic realm="Isolated settings prototype", charset="UTF-8"',
        "Cache-Control": "no-store",
      });
      response.end("Workspace fixture authentication required");
      return;
    }
    response.setHeader("Cache-Control", "no-store");
    if (request.url?.startsWith("/api/")) {
      response.writeHead(403);
      response.end("Browser fixture only; no backend API");
      return;
    }
    next();
  };
  const server = await createServer(
    mergeConfig(config, {
      configFile: false,
      envDir: directory,
      plugins: [
        {
          name: "workspace-fixture-access",
          configureServer(vite) {
            vite.middlewares.use(protect);
          },
        },
      ],
      server: {
        host: "127.0.0.1",
        port,
        strictPort: true,
        open: false,
        hmr: false,
        allowedHosts: ["preview2.luckyvibekanban.com"],
        cors: { origin: "https://preview2.luckyvibekanban.com" },
        fs: {
          strict: true,
          allow: [
            "tests/settings-redesign/fixture",
            "packages/web-core/src",
            "packages/local-web/src",
            "packages/ui/src",
            "packages/web-core/node_modules",
            "packages/local-web/node_modules",
            "packages/ui/node_modules",
            "node_modules",
            "shared",
          ].map((value) => path.join(repo, value)),
          deny: [
            "**/.build/**",
            "**/.git/**",
            "**/.env*",
            "**/*.{pem,key,sqlite,sqlite3,db,token}",
            "**/credentials.json",
          ],
        },
      },
    }),
  );
  let stopping = false;
  const stop = async () => {
    if (stopping) return;
    stopping = true;
    await server.close();
  };
  process.once("SIGTERM", () => void stop());
  process.once("SIGINT", () => void stop());
  if (command === "--check") {
    const check = (authorization, url) => {
      let result = 0;
      protect(
        { headers: { authorization }, url },
        {
          writeHead: (status) => {
            result = status;
          },
          end() {},
          setHeader() {},
        },
        () => {
          result = 200;
        },
      );
      return result;
    };
    const assert = (condition) => {
      if (!condition) throw Error("Preview access isolation check failed");
    };
    try {
      assert(check(undefined, "/") === 401);
      assert(check("Basic invalid", "/") === 401);
      assert(check(expected.toString(), "/api/info") === 403);
      assert(check(expected.toString(), "/") === 200);
      assert(!server.config.server.fs.allow.includes(path.resolve(repo)));
      assert(server.config.server.fs.deny.includes("**/.build/**"));
      console.log(
        "Preview configuration and access isolation checks passed; no listener started.",
      );
    } finally {
      await server.close();
    }
    process.exit(0);
  }
  await server.listen();
  console.log(
    "Isolated settings fixture listening on 127.0.0.1:18444 (managed foreground process).",
  );
}
