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

Release profile values in Cargo.toml remain authoritative. Raw Cargo continues
to serve distribution and specialised tooling (SQLx prepare and instance builds
have their own requirements). Do not infer unpublished Project setup/environment
settings: these were not fetched for this task. Repository setup-action code was
read, but no Project or server configuration was changed.

## Scoped verification

The self-contained offline fixture uses the installed pinned toolchain, no
registry dependencies, two source workspaces, one temporary shared build-dir and
independent targets. It verifies debug/test flags, nonincremental compilation,
release debug preservation, fallback without shared storage/sccache, wrapper and
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

LOCAL-178/182 formal Integration takes priority. Do not run full Rust builds,
full pnpm validation, installs or a cold-cache experiment until authorised.

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
