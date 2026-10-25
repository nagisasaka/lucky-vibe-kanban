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

check_hash .build/local32-additional-artifacts/server 285cb1a4c249147fc8f3b2da35ca3c835f1153ec25eeecf0533bca66118686fe
check_hash .build/local32-additional-artifacts/agent-process-host e9bb962e05c0c4e6754bb7770b1640e01085f5ad839362ec4885507ea9f6515e
check_hash packages/local-web/dist/index.html e29e03216868077bb3c8efcec3f790500aa7439751d5f9b7664ec97603b05c06
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
export VIBE_KANBAN_AGENT_PROCESS_HOST="$preview_root/.build/local32-additional-artifacts/agent-process-host"

exec "$preview_root/.build/local32-additional-artifacts/server"
