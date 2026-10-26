# LVK development Cargo cache

The contributor entry is `node scripts/dev-cargo.mjs check --workspace` (or the
existing pnpm backend scripts). It is scoped to the checkout containing the
script. Cargo settings are command arguments; only rustc's sccache subprocess
receives the additional cache environment. No product runtime, DB/API, Project
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
or 0; conflicting opt/debug/strip/split/incremental profile variables, Rust flags,
wrappers and target overrides are rejected with an explanation. The build entry
does not inject these variables into run/watch or managed child Projects.
Instance source snapshots use their own target directories. If a snapshot has no
`.lvk-shared/cache`, the documented local fallback applies; it does not reach into
another checkout to discover shared storage.

## Scoped verification

The self-contained offline fixture uses the installed pinned toolchain, no
registry dependencies, two source workspaces, one temporary shared build-dir and
independent targets. It verifies debug/test flags, nonincremental compilation,
all three purpose profiles and final ELF line tables, fallback without shared storage/sccache, wrapper and
environment conflicts, run's environment, Cargo lock contention and raw clean's
impact. A fake sccache transport forwards to real rustc to test environment scope
without changing the shared server. This is not a real sccache hit-rate benchmark.
`LVK_TEST_REAL_SCCACHE=1 node --test scripts/dev-cargo.test.mjs` also compiles a
dependency-free library through the installed real sccache; both tests passed
on the pinned toolchain. It uses the existing server without reconfiguration.
The fixture's clean only touches its own disposable cache. Cargo's shared build
lock serialises builds even when their final target directories differ. A raw
clean removes shared intermediates but leaves the other target's final binary;
that binary's existence does not mean its intermediate cache survived.

The existing server's observed cache location matched canonical
`.lvk-shared/cache/sccache`; its configuration and statistics were not reset.
Actual LVK builds and real cache performance remain unverified.

## Deferred measurement (requires capacity/execution-slot approval)

The previous LOCAL-178/182 execution-slot wait is historical and ended. For
LOCAL-197 the CODE run is limited to implementation and small local fixtures.
The integration owner handles the single heavy dev artifact build after commit;
full validation is explicitly skipped at Integration admission. Cold/warm
benchmarks still require separate capacity/execution approval.

1. Record candidate SHA, toolchain, command/features, free disk, selected target
   usage, expected/actual sccache location and `sccache --show-stats`. Coordinate
   the full resource bundle before any registered shared-resource use.
2. With an approved task-owned empty cache/build-dir fixture or coordinated
   maintenance window, measure a cold LVK debug build. Never clear or restart the
   existing shared sccache server to manufacture a cold baseline. Document the
   distinction between cold Cargo artifacts and a warm shared compiler cache.
3. Repeat the identical command warm, then make a small reversible Rust edit and
   rebuild, then build from a second independent worktree sharing intermediates.
   Keep final targets separate and restore the edit. Compare against a separately
   authorised incremental baseline if a policy comparison is required.
4. For each case record elapsed time, target and intermediate byte deltas, and
   hits/misses/non-cacheable reason deltas. Shared sccache counters include other
   clients; record concurrent activity and avoid attributing all deltas to LVK.
5. Verify actual runtime/managed child-Project environment and watch lifecycle in
   an authorised isolated LVK instance. Fixture evidence is not live acceptance.

These cold/warm/edit/second-workspace timings and capacity savings have **not**
been measured. Retain results with the candidate SHA and command; do not report
fixture success as a completed LVK benchmark or formal Integration.

## LOCAL-197 local verification (2026-10-26)

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
