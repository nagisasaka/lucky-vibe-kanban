#!/usr/bin/env bash
# Run alone: instance paths and the no-model CLI trap are process-wide.
set -euo pipefail
cd "$(dirname "$0")/../.."
admission_instance=$(mktemp -d /tmp/lvk-openwiki-admission-XXXXXX)
trap 'rm -rf -- "$admission_instance"' EXIT
LVK_INSTANCE_DIR="$admission_instance" \
  LVK_ACCESS_DEPLOYMENT=local LVK_AUTH_MODE=local \
  PATH="$admission_instance/bin:$PATH" \
  node scripts/dev-cargo.mjs test -p server \
  recovery_never_admits_for_publication_pending_backlog_or_direct_main --lib -- \
  --ignored --exact \
  routes::openwiki::recovery_tests::recovery_never_admits_for_publication_pending_backlog_or_direct_main
