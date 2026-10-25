#!/usr/bin/env bash
# LOCAL-32's reviewed foreground entrypoint for LVK's regular DevServer owner.
# Root coordinates stopping the old preview and placing the frozen assets first.
set -euo pipefail

preview_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$preview_root"

check_hash() {
  local actual
  actual="$(sha256sum -- "$1")"
  if [[ "${actual%% *}" != "$2" ]]; then
    printf 'LOCAL-32 preview artifact mismatch: %s\n' "$1" >&2
    exit 1
  fi
}

# Refuse an accidental fresh instance. Never seed or configure authentication here.
for required in data/db.v2.sqlite data/access.key data/access.configured; do
  if [[ ! -f ".build/local32-user-preview/$required" ]]; then
    printf 'LOCAL-32 retained instance prerequisite missing: %s\n' "$required" >&2
    exit 1
  fi
done

check_hash target/debug/server ff201915b30b2a11d23b0f97b3907ec43101f2195a9a3b862644f4073d092d1e
check_hash target/debug/agent-process-host 0bdb526649d2f8025b8bb6cc8245359ef9aeacaab858d65eec390192650ae2d3
check_hash packages/local-web/dist/index.html 7083012aaa7f5e95610650994a6bf0c859f7f1bcadd9f8a2c5bacf051d14888c
if [[ -n "$(ss -H -ltn 'sport = :18443')" ]]; then
  printf 'LOCAL-32 preview port 18443 is occupied; root must coordinate the cutover.\n' >&2
  exit 1
fi

export LVK_INSTANCE_DIR="$preview_root/.build/local32-user-preview"
export LVK_ACCESS_DEPLOYMENT=public
export LVK_PUBLIC_URL=https://preview1.luckyvibekanban.com
export LVK_AUTH_MODE=
export HOST=127.0.0.1 PORT= BACKEND_PORT=18443 PREVIEW_PROXY_PORT=0
export VK_ALLOWED_ORIGINS=https://preview1.luckyvibekanban.com
export VK_PREVIEW_DOMAIN= DISABLE_WORKTREE_CLEANUP=1 BROWSER=true RUST_LOG=warn
export VIBE_KANBAN_AGENT_PROCESS_HOST="$preview_root/target/debug/agent-process-host"

exec "$preview_root/target/debug/server"
