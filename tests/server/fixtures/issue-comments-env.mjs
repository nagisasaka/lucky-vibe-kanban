import { existsSync, realpathSync } from "node:fs";

// Explicit allowlist: do not propagate live LVK/MCP/DB/provider credentials.
export function isolatedEnvironment(parent, instance, root) {
  const env = {};
  for (const name of [
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "TZ",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "PLAYWRIGHT_BROWSERS_PATH",
    "CARGO_INCREMENTAL",
    "RUSTC_WRAPPER",
    "SCCACHE_SERVER_PORT",
  ]) {
    if (parent[name] !== undefined) env[name] = parent[name];
  }
  const cache = `${root}/.lvk-shared/cache/sccache`;
  return {
    ...env,
    LVK_INSTANCE_DIR: instance,
    LVK_ACCESS_DEPLOYMENT: "local",
    LVK_AUTH_MODE: "local",
    LVK_PUBLIC_URL: "",
    HOST: "127.0.0.1",
    BACKEND_PORT: "0",
    PORT: "0",
    PREVIEW_PROXY_PORT: "0",
    DISABLE_WORKTREE_CLEANUP: "1",
    RUST_LOG: "info",
    CARGO_BUILD_JOBS: "2",
    CARGO_TARGET_DIR: `${root}/target`,
    RUSTC_WRAPPER: parent.RUSTC_WRAPPER ?? "/usr/local/bin/sccache",
    ...(existsSync(cache)
      ? { SCCACHE_DIR: realpathSync(cache) }
      : parent.SCCACHE_DIR
        ? { SCCACHE_DIR: parent.SCCACHE_DIR }
        : {}),
  };
}
