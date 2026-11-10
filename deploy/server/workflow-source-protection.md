# Workflow reporting runtime prerequisites

The three user presets stop after their reports. Their reporting phases require
source write prevention before a provider starts; the later SHA/clean check is
an additional check, not a replacement. Ordinary single-agent execution and
repository-owned Wiki quality/publication policy are separate.

## Image and host boundary

The Dockerfile `server-tools` stage includes `bubblewrap` (`/usr/bin/bwrap`) and
`socat`, inherited by the published `server` target. Build-time version checks
verify the binaries exist; they do not prove the runtime can create namespaces.
A bundle replacement or development-server restart does not install these OS
packages into an existing container. Image construction and container cutover
require their own authorization; this document does not authorize either.

Claude reporting currently requires Linux. Its whole official provider process
runs inside bubblewrap with the Workspace and registered source repositories
mounted read-only. The native Bash sandbox also remains enabled, requires its
dependencies, denies source writes and forbids an unsandboxed retry. File-writing,
agent-delegation and MCP tools are disabled for this phase. Authentication checks
and administrator settings remain in force; arbitrary launcher overrides are
rejected rather than weakening protection.

The runtime must permit the unprivileged user, PID, IPC, UTS and mount namespaces
used by the outer wrapper, its `/proc` mount, and the native sandbox's nested
namespace/network setup. A kernel setting alone does not establish this:
container syscall filters and host LSM policy may still deny it. No particular
capability grant, seccomp exception or host-policy change is prescribed here.
On rejection, retain the error and stop. Do not add privilege or an unsandboxed
fallback merely to make a probe pass.

## Temporary and cache writes

Host-provided shared `tmp`/`cache` roots and the system temporary directory are
explicit native sandbox write locations for both providers. Symlinks are resolved
before source-root checks. Source roots remain denied in the native policy and,
for Claude, mounted read-only around the entire process. Use temporary/cache
locations outside source for build outputs; a build that insists on writing in
the source tree cannot run unchanged in a reporting phase. These additions do
not remove managed restrictions or modify saved provider settings.

## Minimal acceptance check in an authorized environment

Use a disposable Git fixture, not a live Workspace or shared data. Through the
actual reporting adapter, verify a source read succeeds, source create/edit/
rename/unlink attempts fail before modification, and temporary/cache writes
succeed. Confirm the source sentinel, Git diff and HEAD remain unchanged. Then
verify one small provider report reaches its successor once and stops at the
preset boundary. Test missing dependencies separately: no provider may launch
without the required protection. A unit test of settings alone is not this
acceptance check.

At base `367f3cf6`, the development execution environment had neither bwrap nor
socat. A previous Codex sandbox probe reported that a user namespace could not
be created. Read-only observations in this continuation show user namespaces
configured nonzero, `unprivileged_userns_clone=1`, an active seccomp filter and
no effective capabilities, `docker-default (enforce)` AppArmor confinement and
`apparmor_restrict_unprivileged_userns=1`. These facts show multiple active
constraints; they do not identify the exact filter/LSM/kernel check that denied
the earlier operation. The denied probe was not retried, and no security
settings were changed. Successful source protection plus cache writes and real
provider completion remain unverified until the acceptance check can run in an
authorized runtime with the required dependencies and existing policy support.

Official references: [Claude sandbox boundaries and filesystem settings](https://code.claude.com/docs/en/sandboxing)
and [Anthropic sandbox-runtime](https://github.com/anthropics/sandbox-runtime).
