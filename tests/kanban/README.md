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
