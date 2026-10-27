# LVK development Cargo cache

The contributor entry is `node scripts/dev-cargo.mjs check --workspace` (or the
existing pnpm backend scripts). It is scoped to the checkout containing the
script. Cargo settings are command arguments; no compiler-cache wrapper or
additional compiler-cache environment is installed. No product runtime, DB/API, Project
setup script or global Cargo configuration changes are involved.

`cargo run` receives the same settings without exporting them to its binary.
Watch uses `cargo watch -s` to invoke the entry for build and run, rather than
expecting external cargo-watch to propagate Cargo global configuration arguments.
Inherited user environment remains the user's responsibility; the entry does not
sanitise unrelated existing environment or add product environment filters.

## Server purposes

| Purpose | Command (repository root) | Final binaries | Profile |
| --- | --- | --- | --- |
| Preview | `node scripts/dev-cargo.mjs build --locked --bin server --bin agent-process-host` | `target/debug/{server,agent-process-host}` | dev, opt-level=0, line-tables-only |
| Shared development | `node scripts/dev-cargo.mjs --purpose dev build --locked -p server -p local-deployment -p mcp --bin server --bin agent-process-host --bin vibe-kanban-mcp --features server/developer-runtime` | `target/release/{server,agent-process-host,vibe-kanban-mcp}` | release, opt-level=3, line-tables-only |
| Stable container | `docker build --target server .` (GitHub distribution) | `/usr/local/bin/{server,agent-process-host,vibe-kanban-mcp}` in image | release, opt-level=3, debug=0 |

Preview/dev explicitly use strip=none and split-debuginfo=off: line tables stay
in the final binary, so copying/bundling needs no debug sidecar. Stable chef cook
and server build both explicitly set debug=0, strip=symbols, split-debuginfo=off
and incremental=false in command-local environment (never image ENV). The local
`--purpose stable build` entry exercises the same profile without a container.
Ordinary Cargo and Cargo.toml defaults remain unchanged for other consumers.

Build the frontend first for instance/bundle creation; shared dev also needs the
existing developer-runtime feature. No extra compiler environment is required.
CARGO_BUILD_JOBS may limit concurrency. Inherited CARGO_INCREMENTAL must be absent
(including an inherited 0 or 1, which would override the per-profile policy).
`CARGO_BUILD_INCREMENTAL` and configured `build.incremental` also override the
profiles, so both true and false are rejected with a diagnostic. Remove the
override or use raw Cargo deliberately; the entry never edits user config.
Other conflicting opt/debug/strip/split/incremental profile variables, Rust flags,
wrappers and target overrides are rejected with an explanation. The build entry
does not inject these variables into run/watch or managed child Projects.
Instance source snapshots use their own target directories. If a snapshot has no
`.lvk-shared/cache`, the documented local fallback applies; it does not reach into
another checkout to discover shared storage.

## Compiler cache retirement (2026-10-27)

The development entry no longer discovers, invokes or configures sccache. It
uses rustc directly and rejects explicit inherited/configured compiler wrappers
with an explanation, leaving the user's configuration intact. Use raw Cargo
intentionally for a different wrapper policy. Validation helpers no longer add
sccache automatically. General CI setup and the stable container
continue to include sccache for their existing consumers; they are outside this
development-entry policy and are unchanged from the base revision.
This does not uninstall host tools, stop existing daemons, clear caches or delete
shared build-dir/target data. No global Cargo, shell or server-parent environment
changes are involved.

Official Cargo `build.build-dir` sharing, checkout-local final targets, all three
server opt/debug/strip/split profiles remain in place. Preview dev/test now use
incremental=true; common dev/stable release remain incremental=false. These are
Cargo profile settings, never a global CARGO_INCREMENTAL=1 injection. Cargo owns
the shared incremental directory and its existing build lock; no independent
incremental-sharing or locking mechanism is added. Cargo's normal lock may
make builds wait; never use `cargo clean` to maintain the shared build-dir.
Without `.lvk-shared/cache`, intermediates stay in checkout-local `target/`.

This is a KISS decision, not a claim that sccache never hits. The isolated
v0.18.0 investigation reproduced same-condition Rust hits and separate misses
from cwd, absolute input arguments and CARGO_MANIFEST_DIR differences.
SCCACHE_BASEDIRS did not remove Rust cwd sensitivity. The actual server build's
8 misses and historical incremental benefit were not individually attributed.
No replacement compiler-cache framework is introduced.

## Scoped verification

`node --test scripts/dev-cargo.test.mjs` uses the pinned real Cargo/rustc with
small offline fixtures and disposable shared intermediates. It checks the three
profiles, ELF line tables, dev/test incremental=true and release=false,
separate final targets,
shared build-dir/normal lock waiting, local fallback, clean protection, wrapper
conflicts and run/watch environment boundaries. A poisoned sccache executable
on PATH must never be called, including for statistics/probes. No cache daemon
is used. Raw clean is exercised only against the fixture's disposable build-dir.

The previous LOCAL-178/182 execution-slot wait and compiler-cache measurement
plan are historical. Future full build/capacity measurements need their own
execution authorisation; fixture results are not performance benchmarks or live
acceptance. Preserve existing shared caches and running build processes.

## Historical LOCAL-197 local verification (2026-10-26)

Base HEAD/main: `0d73e1ad8eb1790ac896eb958b2757d090ac2cea` plus this change.
Cargo 1.93.0-nightly (bd979347d), rustc 1.93.0-nightly (83e49b75e).
`node --test scripts/dev-cargo.test.mjs deploy/server/instance.test.mjs` passed
4 tests; the opt-in real-sccache smoke was skipped. The fixture verified real
compiler flags and ELF decoded line tables for all three purposes, including
copied dev/stable binaries and Docker's exact command-local profile variables.
It also covered profile/Rust flag/wrapper conflicts, target isolation/shared
intermediates, lock waiting, disposable clean impact and preview/dev run plus
watch environment boundaries. Instance tests used disposable local state.
`node --check deploy/developer/instance.mjs`, `node --check scripts/dev-cargo.mjs`
and `git diff --check` passed. Task-owned fixture directories were removed.

No LVK full build, full pnpm validation, container build, running deployment,
real sccache smoke/benchmark, merge or restart was performed in this CODE run.
Final deployed binaries, embedded payload, build identity, backtrace line
resolution and health remain the integration owner's acceptance responsibility.

## Retirement verification (2026-10-27)

Base: `a3247dadd5b4385e07a7848fd0009aa07d8ffa71` plus this change.
`node --test scripts/dev-cargo.test.mjs tests/server/issue-comment-reviews.test.mjs`
passed all 7 tests (no skips), including the poisoned executable, explicit wrapper
preservation/rejection, isolated helper defaults, real Cargo profiles/ELF line
checks, build-dir sharing/target isolation, lock/clean and runtime boundaries.
The subsequent incremental decision is verified by dev/test compiler incremental
flags, release absence of those flags and rejecting inherited CARGO_INCREMENTAL
0/1 for all three purposes, plus conflicting per-profile environment overrides.
Node syntax checks, Python AST parsing, shell syntax and `git diff --check` passed.

The original retirement run could not load `release.test.mjs` because `yaml`
was missing. The review correction restored Dockerfile, smoke checks, tool docs
and three CI files byte-for-byte from base; no deleted development wrapper is
referenced there, so no exception was needed. A filtered root pnpm install with
the frozen lockfile, ignored scripts and a workspace-local store supplied the
existing yaml dependency without manifest/lockfile changes. The release tests
then passed all 3 cases.

The same review reproduced `CARGO_BUILD_INCREMENTAL=true` and configured
`build.incremental=true` enabling release incremental, and false disabling preview
incremental, on the pinned actual Cargo. The entry now rejects both override
sources and values. Regression cases preserve the config file and check all
three purposes; normal real dev/test compiles still enable incremental while
release compiles do not. General CI and stable-tool availability are preserved.
Full LVK Rust/pnpm validation, container builds, deployment and restart were not
run. No real daemon command or shared-cache operation was performed.
