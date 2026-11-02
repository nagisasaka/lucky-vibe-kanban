#!/usr/bin/env bash
# Compatibility entrypoint: inspect all owned packages; require EN/JA only.
set -euo pipefail
I18N_REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
exec node "$I18N_REPO_ROOT/scripts/check-i18n.mjs" "$@"
