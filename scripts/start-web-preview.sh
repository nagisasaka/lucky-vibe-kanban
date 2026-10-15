#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    printf '%s\n' \
        'Build and serve the local frontend for mobile development testing.' \
        'Usage: pnpm run local-web:preview' \
        'FRONTEND_PORT defaults to 4030; BACKEND_PORT defaults to 4021.' \
        'HOST defaults to 0.0.0.0. The existing backend must already be running.' \
        'Restart this command after source changes to rebuild. No HMR.'
    exit 0
fi
if (( $# > 0 )); then
    printf 'Unknown argument: %s\n' "$1" >&2
    exit 2
fi
export FRONTEND_PORT="${FRONTEND_PORT:-4030}"
export BACKEND_PORT="${BACKEND_PORT:-4021}"
export HOST="${HOST:-0.0.0.0}"
for name in FRONTEND_PORT BACKEND_PORT; do
    value="${!name}"
    if [[ ! "$value" =~ ^[0-9]+$ ]] || (( 10#$value < 1 || 10#$value > 65535 )); then
        printf '%s must be an integer between 1 and 65535.\n' "$name" >&2
        exit 2
    fi
done
if (( 10#$FRONTEND_PORT == 10#$BACKEND_PORT )); then
    printf '%s\n' 'FRONTEND_PORT and BACKEND_PORT must be distinct.' >&2
    exit 2
fi
cd -- "$REPO_ROOT"
# Keep preview output separate from the assets embedded by Cargo builds.
NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=4096}" \
    pnpm --dir packages/local-web run build --outDir dist-preview
printf 'Built frontend: http://localhost:%s (API proxy to 127.0.0.1:%s)\n' "$FRONTEND_PORT" "$BACKEND_PORT"
exec pnpm --dir packages/local-web run preview --outDir dist-preview \
    --host "$HOST" --port "$FRONTEND_PORT" --strictPort
