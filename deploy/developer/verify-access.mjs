// CI-only: configure a fresh disposable image through its real HTTPS setup API.
// No fixture secret is printed; the self-signed fixture CA is explicitly trusted.
import assert from "node:assert/strict";
import https from "node:https";
import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
const hostname = process.env.LVK_HOST;
const password = process.argv[2];
assert(hostname && password && password.length >= 12);
const ca = await readFile("/run/lvk-secrets/tls/fullchain.pem");
async function request(path, body, headers = {}) {
  return new Promise((resolve, reject) => {
    const req = https.request(
      {
        hostname,
        port: 8443,
        path,
        method: body ? "POST" : "GET",
        ca,
        lookup: (_name, options, cb) =>
          options.all
            ? cb(null, [{ address: "127.0.0.1", family: 4 }])
            : cb(null, "127.0.0.1", 4),
        headers: {
          Host: hostname,
          Authorization: `Basic ${Buffer.from(`test:${password}`).toString("base64")}`,
          "Content-Type": "application/json",
          ...headers,
        },
      },
      (res) => {
        let result = "";
        res.on("data", (chunk) => (result += chunk));
        res.on("end", () =>
          resolve({
            status: res.statusCode,
            headers: res.headers,
            body: result ? JSON.parse(result) : null,
          }),
        );
      },
    );
    req.on("error", reject);
    req.end(body ? JSON.stringify(body) : undefined);
  });
}
assert.equal((await request("/api/info")).status, 423);
const key = execFileSync("/usr/local/bin/server", ["setup-token"], {
  encoding: "utf8",
}).trim();
const setup = await request(
  "/api/access/setup",
  { key },
  { "X-LVK-CSRF": "setup" },
);
assert.equal(setup.status, 200);
assert.equal(
  (await request("/api/access/setup", { key }, { "X-LVK-CSRF": "setup" })).status,
  401,
  "A setup key cannot be reused",
);
const cookie = setup.headers["set-cookie"][0].split(";")[0];
const result = await request(
  "/api/access/basic",
  { username: "test", password },
  { Cookie: cookie, "X-LVK-CSRF": setup.body.csrf },
);
assert.equal(result.status, 200);
assert.equal((await request("/api/info")).status, 200);
assert.equal((await request("/mcp", {})).status, 404);
console.log(
  "PASS: public locked setup, CLI one-use key, backend Basic, remote MCP disabled (disposable image)",
);
