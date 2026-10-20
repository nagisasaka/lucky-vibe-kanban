# TODO category layout validation — 2026-10-20

Target TODO: `66f2ea58-2905-4acf-8c12-e67065fb3d8a`

Workspace: `366a0d89-23cc-4cf9-b84b-754fb4d3df19`

## Change and ordering decision

The previous `TodoPanel.tsx` used an ordinary auto-fit grid: categories in the
next row waited for the tallest category in the previous row. The requirement
was therefore not already satisfied. The same column-width rule now uses sparse
one-pixel grid rows and measured row spans. A ResizeObserver watches natural
content height, including edit forms, wrapping and deletion confirmation.

Category sorting, item order, DOM/Tab order and existing mutations are unchanged.
Read categories by their top positions, left-to-right for equal top positions.
Dense backfilling and CSS columns were rejected because they can change that
reading order. No model, schema, API or dependency was added. The workspace panel
remains a single column. There is no existing category-collapse action; expansion
was exercised through editing, textarea resizing and deletion confirmation.

## Commands and results

All commands ran from this workspace's repository root.

| Command | Result |
| --- | --- |
| `pnpm --dir packages/local-web run build` | PASS, including its TypeScript stage and production bundle |
| `cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp` | PASS; workspace-local `target/`, instructed shared sccache, no profile/parallelism changes |
| `pnpm run web-core:check` | PASS |
| `pnpm run local-web:check` | PASS on the standalone rerun |
| `pnpm run local-web:lint` | PASS |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/features/todos/ui/TodoPanel.tsx --config .eslintrc.cjs --no-ignore --max-warnings 0` | PASS |
| `node --check tests/todos/layout.mjs` and `node --check tests/todos/isolated.mjs` | PASS |
| `LVK_CHROMIUM_EXECUTABLE=/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome node tests/todos/isolated.mjs` | PASS on final run, exit 0 |
| `git diff --check` | PASS |

An initial concurrent standalone local-web type check was intentionally stopped
(exit 143) after the build's TypeScript stage had passed; its later standalone
rerun passed. Build warnings included existing Browserslist age, Tailwind content
configuration and bundle-size warnings. No dependency/configuration changes were
made to suppress them.

## Real-backend E2E observations

`isolated.mjs` launched the newly built server on `127.0.0.1` with automatically
assigned main/preview ports and a fresh task-owned `LVK_INSTANCE_DIR` containing
its own SQLite/data/runtime. It served this workspace's built frontend. Chromium
used an isolated browser context. REST, SQLite and stdio MCP were real; **no API
or browser-route mocks were used**. No agent execution was started.

- 24 additional fixture categories plus 7 existing fixture categories produced
  31 categories with mixed heights (30-line text, short text, unbroken long text).
- At 1600px the layout had 4 columns; the next category began **678px before** the
  tall first category ended, demonstrating that the old row-sized gap is gone.
- At 1000px there were 2 columns; at 390px and 320px there was 1 column. Returning
  to 1600px restored 4 columns.
- Pairwise category rectangles never overlapped; their bounds fit the container,
  their bottom edges fit its scrollable extent, and the document had no horizontal
  overflow. Category top positions retained source order; item DOM order matched
  the API's order. Tab moved to the next category's checkbox.
- Editing a short item, resizing its textarea to 600px, saving 20 lines, and
  opening deletion confirmation all caused safe relayout. The last fixture
  category could be edited and cancelled at every tested width.
- Completion hid a category (31 → 30); showing completed restored it (30 → 31).
  Deleting the last item removed its category (31 → 30) and GET returned 404.
- The existing suite also passed UI/API/MCP CRUD, stale revision rejection,
  unsent draft preservation, workspace/project navigation, related-card links,
  mobile workspace display, dark/light rendering and reload persistence.
- Browser runtime errors: none. Wide, narrow and light-theme screenshots were
  also visually inspected.

The first E2E attempt passed all width/edit layout checks but failed on the new
test's `checkbox.check()` immediate-state assertion. This controlled checkbox is
updated asynchronously and can disappear after completion. The test now follows
the existing suite's `click()` plus awaited UI-result pattern. The full rerun
passed; the initial attempt is retained as a failure, not counted as a pass.

## Evidence, cleanup and limits

Local evidence (not committed): `.build/todo-validation/e2e.log`,
`e2e-attempt-1.log`, build/type/lint logs and `screenshots/layout-mixed.png`,
`layout-{1600,1000,390,320}.png`, project/workspace dark/light/mobile screenshots.
The first failure screenshot is explicitly named `attempt-1-failure.png`.

After the successful run, browser/server/MCP shutdown completed, no task instance
processes remained, and all `instance-*` test directories were removed. Logs and
screenshots are about 1.3 MB. Normal reusable build output is retained in
`target/` (about 13 GB) and `packages/local-web/dist/` (about 77 MB), per repository
instructions. No live dev server, nginx, other workspace, merge/deploy or target
TODO completion operation was changed.

Firefox, WebKit, physical mobile devices, external OAuth/ChatGPT and the full
Rust test suite were not exercised. This change is frontend-only. There are no
outstanding specification questions; preserving reading order takes precedence
over dense backfilling of every possible gap.
