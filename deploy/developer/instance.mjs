#!/usr/bin/env node
// Optional LVK contributor tooling. Not part of the installed server entrypoint.
import { spawn, execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile, copyFile, access } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { supervise, authority, serverSettings } from "../server/server.mjs";

const script = fileURLToPath(import.meta.url);
export function configuration(name, port, env = process.env) {
  if (!/^[a-z][a-z0-9-]{0,47}$/.test(name))
    throw new Error("Invalid instance name");
  const staging = name === "staging";
  const selected = Number(port ?? (staging ? 4020 : 0));
  if (
    !Number.isInteger(selected) ||
    selected < 1024 ||
    selected > 65535 ||
    [3000, 3001, 8443, 8444].includes(selected) ||
    (staging ? selected !== 4020 : selected === 4020)
  ) {
    throw new Error("Choose a separate internal port; staging uses 4020");
  }
  const root = resolve(
    env.LVK_INSTANCES_ROOT || "/var/tmp/lvk-instances",
    name,
  );
  return { name, staging, port: selected, root };
}
export function environment(config, build, env = process.env) {
  const origin = config.staging
    ? new URL(
        env.LVK_STAGING_ORIGIN ||
          `https://${authority(serverSettings(env).app)}:8443`,
      ).origin
    : `http://127.0.0.1:${config.port}`;
  return {
    ...env,
    LVK_INSTANCE_DIR: config.root,
    LVK_ACCESS_DEPLOYMENT: config.staging ? "public" : "local",
    LVK_AUTH_MODE: config.staging ? env.LVK_AUTH_MODE || "" : "",
    LVK_PUBLIC_URL: config.staging ? origin : "",
    HOST: "127.0.0.1",
    PORT: "",
    BACKEND_PORT: String(config.port),
    PREVIEW_PROXY_PORT: "0",
    VK_ALLOWED_ORIGINS: origin,
    VK_PREVIEW_DOMAIN: "",
    VIBE_KANBAN_AGENT_PROCESS_HOST: join(build, "agent-process-host"),
    TMPDIR: join(config.root, "tmp"),
    BROWSER: "true",
  };
}
async function run(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      detached: true,
      ...options,
    });
    const stop = () => {
      if (child.pid) {
        try {
          process.kill(-child.pid, "SIGTERM");
        } catch (error) {
          if (error.code !== "ESRCH") throw error;
        }
      }
    };
    process.on("SIGINT", stop);
    process.on("SIGTERM", stop);
    const cleanup = () => {
      process.off("SIGINT", stop);
      process.off("SIGTERM", stop);
    };
    child.once("error", (error) => {
      cleanup();
      reject(error);
    });
    child.once("exit", (code, signal) => {
      cleanup();
      if (code === 0) resolvePromise();
      else reject(new Error(`${command} failed (${signal ?? code})`));
    });
  });
}
const git = (cwd, ...args) =>
  execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
async function maintenanceCheck() {
  try {
    await access("/run/lvk-maintenance/active");
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  throw new Error(
    "A stable update is in progress; do not start instances or builds",
  );
}
async function main() {
  let [action, name, argument, port] = process.argv.slice(2);
  if (!["build", "run", "_build", "_run"].includes(action) || !name) {
    throw new Error(
      "Usage: instance.mjs build NAME [REF] | run NAME BUILD_ID [PORT]. staging defaults to main / port 4020. Runs stay in the foreground.",
    );
  }
  const config = configuration(
    name,
    action.endsWith("build") ? (name === "staging" ? 4020 : 4100) : port,
  );
  await mkdir(config.root, { recursive: true, mode: 0o700 });
  await maintenanceCheck();
  if (!action.startsWith("_")) {
    // Kernel releases the lock on process exit, including crashes. Build and run
    // locks are separate so a candidate can be built while the old one serves.
    await run("flock", [
      "--nonblock",
      "--no-fork",
      join(config.root, `${action}.lock`),
      process.execPath,
      script,
      `_${action}`,
      name,
      ...(argument ? [argument] : []),
      ...(port ? [port] : []),
    ]);
    return;
  }
  if (action === "_build") {
    const repo = git(process.cwd(), "rev-parse", "--show-toplevel");
    const id = randomUUID();
    const build = join(config.root, "builds", id);
    await mkdir(build, { recursive: true });
    let source = repo;
    const revision = git(
      repo,
      "rev-parse",
      "--verify",
      `${argument || (config.staging ? "main" : "HEAD")}^{commit}`,
    );
    const dirty =
      !config.staging && !argument && git(repo, "status", "--porcelain").length > 0;
    if (config.staging || argument) {
      source = join(build, "source");
      await run("git", ["worktree", "add", "--detach", source, revision], {
        cwd: repo,
      });
      await run("pnpm", ["install", "--frozen-lockfile"], { cwd: source });
    }
    await access(join(source, "crates/utils/src/instance.rs"));
    const env = {
      ...process.env,
      NODE_OPTIONS: process.env.NODE_OPTIONS || "--max-old-space-size=4096",
      // Freeze the same identity recorded below in build.json into the binary.
      LVK_BUILD_REVISION: revision,
      LVK_BUILD_KIND: "development",
      LVK_BUILD_RELEASE_TAG: "",
      LVK_BUILD_DIRTY: String(dirty),
      CARGO_TARGET_DIR: join(config.root, "target"),
      CARGO_PROFILE_DEV_DEBUG: "0",
      CARGO_INCREMENTAL: "0",
      CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "2",
    };
    // Build UI first: RustEmbed serves it in both debug and release profiles.
    await run("pnpm", ["--dir", "packages/local-web", "run", "build"], {
      cwd: source,
      env,
    });
    await run(
      "cargo",
      ["build", "--locked", "--bin", "server", "--bin", "agent-process-host"],
      { cwd: source, env },
    );
    for (const binary of ["server", "agent-process-host"]) {
      await copyFile(
        join(env.CARGO_TARGET_DIR, "debug", binary),
        join(build, binary),
      );
    }
    await writeFile(
      join(build, "build.json"),
      JSON.stringify(
        {
          id,
          revision,
          source,
          dirty,
          profile: "debug",
          created_at: new Date().toISOString(),
        },
        null,
        2,
      ) + "\n",
    );
    console.log(
      JSON.stringify({ instance: name, build_id: id, revision, dirty }),
    );
    return;
  }
  if (!/^[a-f0-9-]{36}$/.test(argument || ""))
    throw new Error("Expected a build UUID");
  const build = join(config.root, "builds", argument);
  const metadata = JSON.parse(
    await readFile(join(build, "build.json"), "utf8"),
  );
  if (metadata.id !== argument) throw new Error("Build identity mismatch");
  await mkdir(join(config.root, "tmp"), { recursive: true, mode: 0o700 });
  // The foreground supervisor scopes signals to this instance's process group.
  // Source snapshots are retained because debug RustEmbed reads their assets.
  console.log(
    JSON.stringify({ instance: name, port: config.port, ...metadata }),
  );
  process.exitCode = await supervise(
    [[join(build, "server")]],
    environment(config, build),
  );
}
if (process.argv[1] && resolve(process.argv[1]) === script) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
