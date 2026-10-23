# Resource observation validation

Run from the repository root, with dependencies installed using the pinned pnpm
lockfile. Run heavy commands serially and respect the current resource allocation.

```sh
node --test tests/resource-observation/model.test.mjs
node tests/resource-observation/browser.mjs
CARGO_BUILD_JOBS=1 cargo test -p server resource_observation --lib -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p utils instance::tests --lib -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p server resource_observation::tests::live_smoke --lib -- --ignored --exact --test-threads=1 --nocapture
CARGO_BUILD_JOBS=1 cargo clippy -p server --all-targets -- -D warnings
```

Use the repository's sccache configuration for Cargo commands. The smoke test is
a single small read of the current Linux environment; do not add load, OOM tests,
mounts or host configuration changes. The ordinary HTTP access regression also
performs one small read through the shared collector with a disposable auth DB.

The model fixtures check the exact 70/80 boundaries, available-based RAM, invalid
and missing values, distinct filesystem IDs, partial errors and stale timing.
Rust fixtures check counter parsing, unlimited limits, quotas/cpusets, sampling,
timeout with retained partial disk values, admission bounds, filesystem arithmetic,
missing destinations and the MCP/Web authentication boundaries.

The Chromium test bundles the real Navbar, ResourceSummary and polling hook. It
serves them on a temporary loopback port with fixture API replies; external
requests are blocked. Two query subscribers exercise shared fetching. It checks
normal, warning, high, failed, stale and missing states, light/dark themes,
320px workspace/project headers, visible polling, hidden-page suspension and
in-flight deduplication. Screenshot capture waits for tooltip opacity to settle.
The browser, private server and temporary CSS files are closed/removed in `finally`.

Evidence is written to `test-results/resource-observation/` (or the explicit
`RESOURCE_EVIDENCE_DIR`): `results.json` and named PNG screenshots. Keep these local
artifacts for review; do not interpret fixture CPU/memory/disk values as real
measurements. This does not prove a deployed full-app session, real Auth0/ChatGPT
connectivity, multiple real mounted filesystems, or non-Linux behaviour. Independent
review is a separate step managed by the coordinating AI.
