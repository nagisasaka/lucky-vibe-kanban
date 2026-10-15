import { spawn } from "node:child_process";
import { readFile, writeFile, mkdir, realpath } from "node:fs/promises";
import {
  authority,
  serverSettings,
  withMcpIngress,
  renderNginx,
  validateSecrets,
  certificateReloader,
  supervise,
} from "./server.mjs";

export function developmentEnvironment(env, settings) {
  const port = env.LVK_DEV_HTTPS_PORT || "8443";
  if (!/^\d+$/.test(port) || Number(port) < 1024 || Number(port) > 65535)
    throw new Error("LVK_DEV_HTTPS_PORT must be between 1024 and 65535");
  // Vite dev exposes source/filesystem and editor endpoints outside Rust's
  // access boundary. Only serve built assets once the proxy guard is removed.
  const backendOnly = settings.managedAccess && !settings.legacyBasic;
  const frontendMode = env.FRONTEND_MODE || (backendOnly ? "preview" : "dev");
  if (backendOnly && frontendMode !== "preview")
    throw new Error(
      "Backend authentication requires FRONTEND_MODE=preview; Vite dev needs the legacy proxy guard",
    );
  return {
    ...env,
    FRONTEND_MODE: frontendMode,
    LVK_ACCESS_DEPLOYMENT: "public",
    LVK_PUBLIC_URL:
      env.LVK_PUBLIC_URL || `https://${authority(settings.app)}:${port}`,
    HOST: "127.0.0.1",
    PORT: "",
    FRONTEND_PORT: "4020",
    BACKEND_PORT: "4021",
    PREVIEW_PROXY_PORT: "4022",
    VK_ALLOWED_ORIGINS: `https://${authority(settings.app)}:${port}`,
    VK_PREVIEW_DOMAIN: "",
    VITE_VK_SHARED_API_BASE: "",
    VK_SHARED_API_BASE: "",
    BROWSER: "true",
    CARGO_BUILD_JOBS: env.CARGO_BUILD_JOBS || "2",
    CARGO_PROFILE_DEV_DEBUG: "0",
    CARGO_INCREMENTAL: "1",
    CARGO_TARGET_DIR: "/var/tmp/lvk-target",
  };
}

const run = (command, args, env) =>
  new Promise((resolve, reject) => {
    const child = spawn(command, args, { env, stdio: "inherit" });
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`${command} exited ${code}`)),
    );
  });

async function main() {
  const settings = await withMcpIngress(
    {
      ...serverSettings({ ...process.env, LVK_ACCESS_DEPLOYMENT: "public" }),
      preview: null,
    },
    process.env,
  );
  const repo = await realpath(
    process.env.LVK_DEV_REPO || "/repos/lucky-vibe-kanban",
  );
  if (!repo.startsWith("/repos/"))
    throw new Error("LVK_DEV_REPO must be inside /repos");
  await readFile(`${repo}/package.json`);
  process.chdir(repo);
  const read = async () => {
    const [certificate, key, passwords] = await Promise.all([
      readFile("/run/lvk-secrets/tls/fullchain.pem"),
      readFile("/run/lvk-secrets/tls/privkey.pem"),
      readFile("/run/lvk-secrets/htpasswd", "utf8").catch((error) => {
        if (error.code === "ENOENT" && process.env.LVK_PROXY_AUTH !== "legacy")
          return null;
        throw error;
      }),
    ]);
    return { certificate, key, passwords };
  };
  const initial = await read();
  settings.legacyBasic =
    process.env.LVK_PROXY_AUTH !== "backend" && initial.passwords !== null;
  const env = developmentEnvironment(process.env, settings);
  validateSecrets(
    initial.certificate,
    initial.key,
    initial.passwords,
    settings,
  );
  for (const name of ["client", "proxy", "fastcgi", "uwsgi", "scgi"])
    await mkdir(`/tmp/lvk-server/${name}`, { recursive: true, mode: 0o700 });
  const template = await readFile(
    new URL("./nginx.conf.template", import.meta.url),
    "utf8",
  );
  const config = "/tmp/lvk-server/nginx.conf";
  await writeFile(
    config,
    renderNginx(template, settings, { mcpBackend: 4021 })
      .replaceAll("http://127.0.0.1:3000", "http://127.0.0.1:4020")
      .replace("private development server", "source development server"),
    { mode: 0o600 },
  );
  const nginx = (args) =>
    run("nginx", ["-e", "stderr", ...args, "-c", config], env);
  await nginx(["-t"]);
  // Installation is cached in the persistent home; no application image build.
  await run("pnpm", ["install", "--frozen-lockfile"], env);
  const reload = certificateReloader({
    read,
    settings,
    initial,
    activate: async () => {
      await nginx(["-t"]);
      await nginx(["-s", "reload"]);
    },
  });
  const timer = setInterval(
    () => reload().catch((error) => console.error(error.message)),
    30_000,
  );
  try {
    process.exitCode = await supervise(
      [
        ["pnpm", "run", "dev:container"],
        ["nginx", "-e", "stderr", "-c", config, "-g", "daemon off;"],
      ],
      env,
    );
  } finally {
    clearInterval(timer);
  }
}

if (process.argv[1] && new URL(import.meta.url).pathname === process.argv[1]) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
