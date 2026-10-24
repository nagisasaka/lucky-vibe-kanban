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
and missing values, physical used/total/percent consistency, duplicate or absent
worktree observations, partial errors and stale timing.
Rust fixtures check counter parsing, unlimited limits, quotas/cpusets, sampling,
timeout with retained partial disk values, admission bounds, physical filesystem arithmetic,
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

## Isolated full application acceptance

`full-app.mjs` supplements the component fixtures with the actual built app,
`NavbarContainer`, production CSS, full server router and local Basic login. Build
only if the existing artifacts are missing or stale, serially with the repository's
sccache settings and one Cargo job:

```sh
GOMAXPROCS=1 UV_THREADPOOL_SIZE=1 NODE_OPTIONS=--max-old-space-size=4096 pnpm --dir packages/local-web run build
CARGO_BUILD_JOBS=1 cargo build -p server --bin server
node tests/resource-observation/full-app.mjs
```

Prerequisites: installed Chromium for Playwright, OpenSSL, and free loopback ports.
The test creates its own DB, data/worktree-base directories and one-day TLS
certificate under a private temporary directory. It enables the normal public
access policy on this loopback-only instance to exercise Basic authentication and
Secure session cookies; it does not access Auth0 or an existing LVK server. Keys
and passwords stay in memory/private temporary files. Browser external requests
are blocked. No agent run or Integration is started.

Real Chromium requests verify resource API authentication and worktree disk values reaching
the tooltip. Project root, issue subroute and workspace pages use the actual right
icons and user slot at 320/767/768/1280px. Explicit resource-response overrides cover
100%, 503, a six-second delay and a 44-second-old observation crossing the 45-second
stale boundary with the real hook timer. Hidden/visible polling uses an overridden
`document.visibilityState` plus `visibilitychange`, with 17 seconds of real waiting;
it is not an OS tab-backgrounding test. SPA navigation checks query deduplication.
The seeded workspace has no repository/agent execution; unrelated Git/file panels
can report their normal missing-repository errors.

Evidence goes to `test-results/resource-observation/full-app/` (override with
`RESOURCE_APP_EVIDENCE`): results, header/tooltip screenshots and server log.
`finally` closes Chromium, waits for its own server to exit, closes the TLS proxy
and removes its private instance. It never stops an existing server. This is
isolated full-app acceptance, not deployment or formal Integration validation.
