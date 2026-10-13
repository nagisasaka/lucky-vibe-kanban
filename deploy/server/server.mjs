import { spawn } from "node:child_process";
import { X509Certificate, createPrivateKey, createHash } from "node:crypto";
import { isIP } from "node:net";
import { constants } from "node:fs";
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

export function domain(value, name) {
  if (
    typeof value !== "string" ||
    value.length > 253 ||
    !/^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(
      value,
    )
  ) {
    throw new Error(
      `${name} must be a lowercase DNS hostname, without scheme, port, or path`,
    );
  }
  return value;
}

export function serverSettings(env) {
  const value = env.LVK_HOST;
  const version = isIP(value ?? "");
  // Browsers canonicalise IPv6 literals in Host and Origin headers.
  const app =
    version === 6
      ? new URL(`https://[${value}]`).hostname.slice(1, -1)
      : version === 4
        ? value
        : domain(value, "LVK_HOST");
  const preview = env.LVK_PREVIEW_DOMAIN
    ? domain(env.LVK_PREVIEW_DOMAIN, "LVK_PREVIEW_DOMAIN")
    : null;
  if (preview && (app === preview || app.endsWith(`.${preview}`))) {
    throw new Error("LVK_HOST must be outside the preview wildcard domain");
  }
  return {
    app,
    preview,
    ...(env.LVK_ENABLE_STAGING === "1" ? { staging: true } : {}),
  };
}

export const authority = (host) => (isIP(host) === 6 ? `[${host}]` : host);

// Read the same opt-in file as Rust. OAuth validation remains in the backend;
// this only validates the public route before removing its Basic challenge.
export async function withMcpIngress(settings, env, read = readFile) {
  if (!env.LVK_MCP_CONFIG) return settings;
  let base;
  try {
    const config = JSON.parse(await read(env.LVK_MCP_CONFIG, "utf8"));
    base = new URL(config.public_base_url);
    if (
      base.protocol !== "https:" ||
      base.hostname !== authority(settings.app) ||
      base.username ||
      base.password ||
      base.search ||
      base.hash ||
      config.public_base_url.endsWith("/") ||
      base.href !==
        config.public_base_url + (base.pathname === "/" ? "/" : "") ||
      !/^(?:\/[A-Za-z0-9_-]+)*\/?$/.test(base.pathname) ||
      /^\/(?:api|assets|src|mcp)(?:\/|$)/.test(base.pathname)
    )
      throw new Error();
  } catch {
    throw new Error(
      "MCP ingress requires a readable config with a canonical HTTPS public_base_url on LVK_HOST and a plain path prefix",
    );
  }
  return {
    ...settings,
    mcp: { prefix: base.pathname === "/" ? "" : base.pathname },
  };
}

function mcpLocations(settings, backend) {
  if (!settings.mcp) return "";
  // A location-level proxy_set_header overrides nginx's inherited header list.
  // Repeat the complete boundary here, including relay/Basic credential removal.
  return ["/mcp", "/.well-known/oauth-protected-resource/mcp"]
    .map(
      (path) => `
        location = ${settings.mcp.prefix}${path} {
            auth_basic off;
            proxy_set_header Host $http_host;
            proxy_set_header X-Forwarded-Proto https;
            proxy_set_header X-Forwarded-Host $http_host;
            proxy_set_header X-Forwarded-For $remote_addr;
            proxy_set_header Authorization ${path === "/mcp" ? "$lvk_mcp_bearer" : '""'};
            proxy_set_header Cookie "";
            proxy_set_header Proxy-Authorization "";
            proxy_set_header X-VK-Relayed "";
            proxy_set_header X-VK-Sig-Session "";
            proxy_set_header X-VK-Sig-Ts "";
            proxy_set_header X-VK-Sig-Nonce "";
            proxy_set_header X-VK-Sig-Signature "";
            proxy_set_header Upgrade "";
            proxy_set_header Connection "";
            proxy_pass http://127.0.0.1:${backend}${path};
        }`,
    )
    .join("\n");
}

// An app alias on the same LVK origin. Root APIs/assets stay on their existing
// authenticated routes; this is not an isolation boundary for another app.
function prefixedAppLocation(settings) {
  const prefix = settings.mcp?.prefix;
  if (!prefix) return "";
  return `
        location ${prefix}/ {
            proxy_set_header Host $http_host;
            proxy_set_header X-Forwarded-Proto https;
            proxy_set_header X-Forwarded-Host $http_host;
            proxy_set_header X-Forwarded-For $remote_addr;
            proxy_set_header Authorization "";
            proxy_set_header Proxy-Authorization "";
            proxy_set_header X-VK-Relayed "";
            proxy_set_header X-VK-Sig-Session "";
            proxy_set_header X-VK-Sig-Ts "";
            proxy_set_header X-VK-Sig-Nonce "";
            proxy_set_header X-VK-Sig-Signature "";
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection $connection_upgrade;
            proxy_set_header Accept-Encoding "";
            sub_filter '<head>' '<head><meta name="lvk-basepath" content="${prefix}">';
            sub_filter_once on;
            proxy_pass http://127.0.0.1:3000/;
        }
        location ~ ^${prefix}/api/(?:host/[^/]+/)?preview(?:/|$) { return 403; }
    `;
}

export function renderNginx(template, settings, { mcpBackend = 3000 } = {}) {
  if (![3000, 4021].includes(mcpBackend))
    throw new Error("Invalid MCP backend");
  const preview = settings.preview;
  const previewServer = preview
    ? `server {
        listen 8443 ssl;
        server_name "~^(?<preview_target>102[4-9]|10[3-9][0-9]|1[1-9][0-9]{2}|[2-9][0-9]{3}|[1-5][0-9]{4}|6[0-4][0-9]{3}|65[0-4][0-9]{2}|655[0-2][0-9]|6553[0-5])[.]${preview.replaceAll(".", "[.]")}$";
        # Only unprivileged development ports, excluding LVK and nginx ports.
        if ($preview_target ~ "^(3000|3001|8443${settings.staging ? "|8444" : ""})$") { return 403; }
        location / { proxy_pass http://127.0.0.1:3001; }
    }`
    : "";
  return template
    .replace("@@APP_DOMAIN@@", authority(settings.app))
    .replace(
      "@@MCP_MAP@@",
      settings.mcp
        ? 'map $http_authorization $lvk_mcp_bearer { default ""; "~*^Bearer [^ ]+$" $http_authorization; }'
        : "",
    )
    .replace("@@MCP_LOCATIONS@@", mcpLocations(settings, mcpBackend))
    .replace("@@PREFIXED_APP@@", prefixedAppLocation(settings))
    .replace("@@PREVIEW_SERVER@@", previewServer)
    .replace(
      "@@STAGING_SERVER@@",
      settings.staging
        ? `server {
        listen 8444 ssl default_server;
        server_name _;
        return 444;
    }
    server {
        listen 8444 ssl;
        server_name ${authority(settings.app)};
        if (-f /run/lvk-maintenance/active) { return 503; }
        add_header X-Content-Type-Options nosniff always;
        add_header X-Frame-Options DENY always;
        location ~ ^/api/(?:host/[^/]+/)?preview(?:/|$) { return 403; }
        location / { proxy_pass http://127.0.0.1:4020; }
    }`
        : "",
    );
}

export function validateSecrets(
  certificate,
  key,
  passwords,
  settings,
  now = Date.now(),
) {
  const cert = new X509Certificate(certificate);
  if (Date.parse(cert.validFrom) > now || Date.parse(cert.validTo) <= now) {
    throw new Error(
      "TLS certificate is not currently valid; renew it before starting",
    );
  }
  for (const host of [
    settings.app,
    ...(settings.preview ? [`3000.${settings.preview}`] : []),
  ]) {
    if (!(isIP(host) ? cert.checkIP(host) : cert.checkHost(host)))
      throw new Error(`TLS certificate does not cover ${host}`);
  }
  if (!cert.checkPrivateKey(createPrivateKey(key)))
    throw new Error("TLS certificate/key mismatch");
  const entries = passwords
    .split(/\r?\n/)
    .filter((line) => line.trim() && !line.startsWith("#"));
  if (
    !entries.length ||
    entries.some(
      (line) =>
        !/^[A-Za-z0-9_.-]+:\$2[aby]\$\d{2}\$[./A-Za-z0-9]{53}$/.test(line),
    )
  ) {
    throw new Error(
      "htpasswd must contain bcrypt entries; generate it with htpasswd -B",
    );
  }
}

// Keep public listeners out of the Rust services, irrespective of inherited env.
export function backendEnvironment(env, settings) {
  return {
    ...env,
    HOST: "127.0.0.1",
    PORT: "",
    BACKEND_PORT: "3000",
    PREVIEW_PROXY_PORT: "3001",
    VK_ALLOWED_ORIGINS: `https://${authority(settings.app)}`,
    VK_PREVIEW_DOMAIN: settings.preview ?? "",
    BROWSER: "true",
  };
}

// A renewed certificate must not restart the backend or its running agents.
// Poll the directory rather than watching an inode replaced by a deploy hook.
export function certificateReloader({ read, activate, settings, initial }) {
  const digest = ({ certificate, key }) =>
    createHash("sha256").update(certificate).update(key).digest("hex");
  let loaded = digest(initial);
  let checking = false;
  return async () => {
    if (checking) return;
    checking = true;
    try {
      const next = await read();
      const fingerprint = digest(next);
      if (fingerprint === loaded) return;
      validateSecrets(next.certificate, next.key, next.passwords, settings);
      await activate();
      loaded = fingerprint;
      console.log("TLS certificate reloaded without restarting agents");
    } finally {
      checking = false;
    }
  };
}

export function supervise(
  commands,
  env,
  {
    spawnProcess = spawn,
    shutdownMs = 120_000,
    signalProcess = process.kill.bind(process),
  } = {},
) {
  if (!commands.length)
    return Promise.reject(new Error("No services configured"));
  return new Promise((resolve) => {
    const children = new Set();
    let stopping = false;
    let result = 0;
    let timeout;
    const kill = (child, signal) => {
      if (child.pid) {
        try {
          signalProcess(-child.pid, signal);
        } catch (error) {
          if (error.code !== "ESRCH") {
            result = 1;
            console.error(
              `Unable to signal service ${child.pid}: ${error.message}`,
            );
          }
        }
      }
    };
    const finish = () => {
      if (!stopping || children.size) return;
      clearTimeout(timeout);
      process.off("SIGTERM", stop);
      process.off("SIGINT", stop);
      resolve(result);
    };
    const stop = () => {
      if (stopping) return;
      stopping = true;
      for (const child of children) kill(child, "SIGTERM");
      timeout = setTimeout(() => {
        for (const child of children) kill(child, "SIGKILL");
      }, shutdownMs);
      finish();
    };
    process.on("SIGTERM", stop);
    process.on("SIGINT", stop);
    for (const [command, ...args] of commands) {
      let child;
      try {
        child = spawnProcess(command, args, {
          env,
          stdio: "inherit",
          detached: true,
        });
      } catch (error) {
        console.error(`Unable to start service ${command}: ${error.message}`);
        result = 1;
        stop();
        break;
      }
      children.add(child);
      child.once("error", () => {
        result = 1;
        children.delete(child);
        stop();
        finish();
      });
      child.once("exit", () => {
        children.delete(child);
        if (!stopping) {
          result = 1;
          stop();
        }
        finish();
      });
    }
  });
}

async function main() {
  const settings = await withMcpIngress(
    serverSettings(process.env),
    process.env,
  );
  const secrets = "/run/lvk-secrets";
  const readSecrets = async () => {
    const [certificate, key, passwords] = await Promise.all([
      readFile(`${secrets}/tls/fullchain.pem`),
      readFile(`${secrets}/tls/privkey.pem`),
      readFile(`${secrets}/htpasswd`, "utf8"),
    ]);
    return { certificate, key, passwords };
  };
  const initial = await readSecrets();
  validateSecrets(
    initial.certificate,
    initial.key,
    initial.passwords,
    settings,
  );
  for (const dir of ["/home/appuser", "/repos", "/var/tmp"])
    await access(dir, constants.W_OK);
  for (const name of ["client", "proxy", "fastcgi", "uwsgi", "scgi"])
    await mkdir(`/tmp/lvk-server/${name}`, { recursive: true, mode: 0o700 });
  const config = "/tmp/lvk-server/nginx.conf";
  const template = await readFile(
    new URL("./nginx.conf.template", import.meta.url),
    "utf8",
  );
  await writeFile(config, renderNginx(template, settings), { mode: 0o600 });
  // nginx opens /dev/stdout while validating its access log. Node's captured
  // child stdio uses sockets that cannot be reopened this way on Linux.
  // Inherit the container's log descriptors for both startup and renewal.
  const runNginx = (args) =>
    new Promise((resolve, reject) => {
      const child = spawn("nginx", ["-e", "stderr", ...args, "-c", config], {
        stdio: "inherit",
      });
      child.once("error", reject);
      child.once("exit", (code) => {
        if (code === 0) resolve();
        else reject(new Error(`nginx ${args.join(" ")} failed (${code})`));
      });
    });
  await runNginx(["-t"]);
  const reload = certificateReloader({
    read: readSecrets,
    settings,
    initial,
    activate: async () => {
      await runNginx(["-t"]);
      await runNginx(["-s", "reload"]);
    },
  });
  const timer = setInterval(() => {
    reload().catch((error) =>
      console.error(`TLS renewal not loaded; will retry: ${error.message}`),
    );
  }, 30_000);
  try {
    process.exitCode = await supervise(
      [
        ["/usr/local/bin/server"],
        ["nginx", "-e", "stderr", "-c", config, "-g", "daemon off;"],
      ],
      backendEnvironment(process.env, settings),
    );
  } finally {
    clearInterval(timer);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`LVK startup failed: ${error.message}`);
    process.exitCode = 1;
  });
}
