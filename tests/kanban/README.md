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

## Touch activation regression

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/touch-guard.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers LVK_TEST_SERVER=/absolute/path/to/compatible/prebuilt/server node tests/kanban/touch.mjs
```

The first command renders only the production click guard in Chromium. It checks
small tap jitter, an excursion beyond 5px followed by a return, cancellation,
multiple fingers, nested controls, and subsequent keyboard/mouse/touch activation.
It is a component fixture, not an application E2E.

The second command uses the existing isolated backend/Vite approach with a fresh
SQLite instance and dynamically assigned loopback ports. The backend executable
is copied into that disposable instance and hash-verified before execution.
It drives the actual
Kanban page with Chromium mobile emulation (390×844, touch enabled) and trusted
CDP touch events. It records native scrolling and event sequences, checks detail
and tag activation, and exercises long-press, mouse and keyboard DnD against the
real API. No API responses are mocked. `.build/touch-validation/` retains the
results, source/backend revision, screenshots and cleanup receipt; the disposable
instance is removed after stopping the browser, Vite and backend. This does not
verify the feel of a physical smartphone or Safari.

See [the touch validation record](TOUCH-VALIDATION.md) for results, event order,
commands, and the separate live DnD projection limitation.

## Server-driven placement motion (LOCAL-259)

Install the workspace's locked dependencies first (`pnpm install --frozen-lockfile`).
Run the small gate/order/synchronisation regressions and the production motion
component in Chromium:

```sh
node --test tests/kanban/snapshot-motion.test.mjs tests/kanban/order.test.mjs tests/kanban/projection.test.mjs tests/kanban/drop-sync.test.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/server-motion.mjs
```

Run the real API path with a compatible prebuilt backend, using the same isolated
instance/port/cleanup contract described above:

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers LVK_TEST_SERVER=/absolute/path/to/compatible/server LVK_SERVER_MOTION_E2E=1 node tests/kanban/isolated.mjs
```

The API test changes rank/status through the private server and refreshes the
existing collection explicitly from the test; it adds no application polling.
It samples intermediate positions of moved and displaced cards, checks 400ms
non-bouncing easing, canonical rank/status, counts, focus, explicit-sort promotion
suppression, reload, a transport failure/recovery and an update during keyboard
cancellation. Visibility boundaries are injected browser events, not proof of
physical tab suspension. The component fixture also covers a cached generation
with a fresh request pending, rapid retargeting, simultaneous reparentings,
535/536px layout boundaries, real scrolling, column resizing, reduced motion,
offscreen targets and nested focus. Fixtures do not exercise real AgentRuns.

Evidence is written under `.build/server-motion-api/` (position samples and
screenshots) and `.build/server-motion-component/`. Keep these outputs out of
Git. Re-run the existing `LVK_DND_REGRESSION=1` isolated suite and `touch.mjs`
when changing the DnD wrapper or input handling. Freeze source files during
browser runs: HMR invalidates persistence/focus observations.

Motion only presents accepted placements. The first fresh snapshot after cache,
visibility or connection recovery is a quiet baseline. A target with either end
outside the viewport is shown at its final location without a long flight;
visible displaced peers can still animate. Column crossings use an inert,
aria-hidden visual copy, clipped below board headers; the real card retains
focus and the copy does not contribute to column counts. Scroll, resize, input,
reduced motion and unmount synchronously remove automatic animation effects.

## Issue detail and split conversation reading surfaces

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/issue-detail.mjs
```

This renders the production Issue panel, sections, Markdown editor, chat Markdown
and composer with inert fixture data, without a backend or provider. The split
conversation shell mirrors ProjectRightSidebarContainer; it does not mount the
live ConversationList or SessionChatBoxContainer. Outputs in `.build/issue-layout/`
cover 320/390/720/960/1200 CSS-pixel panes in light/dark themes, long code/tables,
multiple authors, relationships/sub-issues, keyboard method selection, comment
reply and the unchanged standalone/create width defaults. Screenshots and geometry
are component evidence, not authenticated app, physical mobile or VSCode-host E2E.

The retained section layout uses one Issue inset (`clamp(1rem, 3cqi, 2rem)`) inside
an outer 64rem cap. Split conversation/composer surfaces use the same gutter rule
with a 64rem surface cap; their Markdown's separate 800px cap is disabled only in
that scope. The header remains full width. Creation and standalone chat retain
their existing defaults. Existing palette/typeface tokens remain in use; the main
Issue title is 1.5rem to distinguish it from Markdown section headings. Workflow,
single agent and Arena share a responsive choice grid and explanatory copy,
while their existing handlers are preserved.
