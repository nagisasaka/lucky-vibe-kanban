import http from "node:http";
import https from "node:https";
import { isIP } from "node:net";
import { existsSync } from "node:fs";
import {
  authority,
  serverSettings,
  verifyCertificateIdentity,
} from "./server.mjs";

function check(client, options, expected) {
  return new Promise((resolve, reject) => {
    const req = client.get(options, (res) => {
      res.resume();
      if (client === https) {
        const certificate = res.socket.getPeerCertificate();
        // Flag failed renewals while there is still time to repair them.
        if (!(Date.parse(certificate.valid_to) > Date.now() + 6 * 3600_000)) {
          reject(new Error("Served TLS certificate expires within six hours"));
          return;
        }
      }
      if (res.statusCode !== expected)
        reject(
          new Error(`Expected HTTP ${expected}, received ${res.statusCode}`),
        );
      else resolve();
    });
    req.setTimeout(4000, () => req.destroy(new Error("Healthcheck timed out")));
    req.on("error", reject);
  });
}

try {
  const { app } = serverSettings(process.env);
  const publicHost = new URL(
    process.env.LVK_PUBLIC_URL || `https://${authority(app)}`,
  ).host;
  await Promise.all([
    check(
      http,
      {
        hostname: "127.0.0.1",
        port: 3000,
        path: "/api/health",
        headers: { Host: publicHost },
      },
      200,
    ),
    // Connect locally while verifying the real public-IP/hostname certificate.
    check(
      https,
      {
        hostname: "127.0.0.1",
        checkServerIdentity: (_hostname, certificate) =>
          verifyCertificateIdentity(app, certificate),
        port: 8443,
        path: "/",
        servername: isIP(app) ? "" : app,
        headers: { Host: publicHost },
        rejectUnauthorized: true,
      },
      existsSync("/run/lvk-maintenance/active")
        ? 503
        : process.env.LVK_PROXY_AUTH !== "backend" &&
            existsSync("/run/lvk-secrets/htpasswd")
          ? 401
          : 200,
    ),
  ]);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
