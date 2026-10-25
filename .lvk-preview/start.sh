#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"
server="$repo_dir/target/debug/server"
instance_dir="$repo_dir/.build/kanban-validation/instance-FTz2vi"
data_dir="$instance_dir/data"

[[ -f "$server" && -x "$server" ]] || { echo 'Missing server executable' >&2; exit 1; }
[[ -d "$instance_dir" && -d "$data_dir" ]] || { echo 'Missing existing instance/data directory' >&2; exit 1; }
[[ -f "$data_dir/db.v2.sqlite" ]] || { echo 'Missing existing fixture database' >&2; exit 1; }

listeners="$(ss -H -ltn '( sport = :18444 )')"
[[ -z "$listeners" ]] || { echo 'Port 18444 occupied; refusing to start' >&2; exit 1; }

# DATA_DIR documents the destination; the server resolves LVK_INSTANCE_DIR/data.
# Keep foreground ownership with LVK's Dev Server ExecutionProcess.
exec env -i \
  HOME="$HOME" PATH="$PATH" LANG=C.UTF-8 \
  LVK_INSTANCE_DIR="$instance_dir" DATA_DIR="$data_dir" \
  HOST=127.0.0.1 PORT=18444 \
  LVK_ACCESS_DEPLOYMENT=public LVK_AUTH_MODE=basic \
  LVK_PUBLIC_URL=https://preview2.luckyvibekanban.com \
  "$server"
