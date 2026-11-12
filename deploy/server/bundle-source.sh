#!/bin/sh
# Build-only: fetch public objects independently of the Docker/CI checkout.
set -eu
upstream=${1:?public upstream URL required}
revision=${2:?full build revision required}
destination=${3:?new destination required}
case "$revision" in *[!0-9a-f]*|'') echo 'Expected a full lowercase Git commit ID' >&2; exit 1;; esac
[ "${#revision}" -eq 40 ] || { echo 'Expected a 40-character Git commit ID' >&2; exit 1; }
[ ! -e "$destination" ] || { echo 'Destination already exists' >&2; exit 1; }
# Ignore builder machine configuration, credentials, hooks and object stores.
unset GIT_DIR GIT_INDEX_FILE GIT_WORK_TREE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0
mkdir -p "$destination"
git init --template= "$destination"
git -C "$destination" -c core.hooksPath=/dev/null fetch --no-tags "$upstream" "$revision"
git -C "$destination" -c core.hooksPath=/dev/null checkout --detach "$revision"
[ "$(git -C "$destination" rev-parse HEAD)" = "$revision" ]
[ "$(git -C "$destination" rev-parse --is-shallow-repository)" = false ]
[ -d "$destination/.git" ]
[ ! -e "$destination/.git/objects/info/alternates" ]
# Walk every reachable object; fail on missing/corrupt ancestors instead of
# distributing a checkout whose history only works on the build machine.
git -C "$destination" fsck --full --strict
# Only HEAD + its reachable ancestors are retained; no remote branches/tags.
git -C "$destination" remote add origin "$upstream"
git -C "$destination" config core.hooksPath /dev/null
