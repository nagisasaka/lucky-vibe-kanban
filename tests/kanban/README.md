# Compact Kanban browser validation

Run from the repository root with installed dependencies and Playwright Chromium:

```sh
LVK_TEST_SERVER=/absolute/path/to/compatible/prebuilt/server node tests/kanban/isolated.mjs
```

The test starts a fresh backend instance (ephemeral ports and SQLite) and this
checkout's frontend through Vite. It never connects the frontend to the existing
server. Confirm the backend build revision/schema compatibility before reuse;
`revision.json` records both frontend Git state and backend build identity.
Backend/frontend/browser are stopped in `finally`; the instance is deleted.
The test does not start agents or call paid models. Check resource coordination
before running alongside other heavy work. No Rust build is required.

Outputs: `.build/kanban-validation/` (screenshots, backend logs, source identity,
geometry and results). These are local test artifacts, not canonical wiki files.

Real API coverage includes Issue creation, 1:N Workspace links, expansion,
row-major order, pointer and keyboard moves, saved ordering/status, detail
navigation and responsive geometry. Summary response overlays exercise running,
approval, input, failure, unseen activity and dev-server states; these are
**fixtures**, not evidence of real provider execution or approval transitions.

The browser uses 2560×1440 CSS px with DPR 1.5, corresponding to a 3840×2160
physical display at 150% OS scaling and 100% browser zoom. Additional viewports
are 1280×900 and 390×844. Device scale affects rasterisation; layout is measured
in CSS px. Each column is also resized to 535/536/537/536/535 px to cross the
33.5rem threshold in both directions at the default 16px root size.

## Readability and workspace statistics

`node tests/kanban/readability.mjs` bundles the production components into an
isolated Chromium fixture. It covers body previews, visible workspace cards,
zero/unknown numbers, duration, responsive widths and keyboard/pointer DnD.
This component fixture does not verify the backend or authentication.

The full `isolated.mjs` test now seeds two disposable Git repositories through
`seed-diff-fixture.py` and checks the real summary API and card wiring for
committed, uncommitted, untracked and clean diffs. It also verifies the Navbar
Integration entry. Use a backend built from the changed source for these checks.

For an explicitly requested human preview only, `LVK_KEEP_TEST_INSTANCE=1`
retains a successful test's private fixture instance after all test processes
stop; its absolute path is written to `retained-instance.txt`. Failed instances
are still removed. `python3 tests/kanban/seed-preview.py` adds inert 0/1/4/12
Workspace samples to that stopped instance. It does not create AgentRuns or
copy credentials. Never expose the local-mode test listener: restart the owned
instance in public mode with its exact HTTPS origin and required authentication,
verify locked/423 first, and let the operator configure it. Do not issue or log
setup tokens as part of the test or seed scripts. Keep the instance, source and
built frontend until the user finishes reviewing it.
