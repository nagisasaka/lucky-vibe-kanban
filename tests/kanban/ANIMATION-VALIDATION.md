# LOCAL-237 animation validation

## Scope and reproduction

`KanbanGrid.tsx` applies the sortable translate and transition and enables overlay landing. `KANBAN_MOTION_PRESETS` is the single tuning point for both effects. `VITE_KANBAN_MOTION=crisp` (provisional default) selects 200ms `cubic-bezier(0.2, 0, 0, 1)`; `bouncy` selects 200ms `cubic-bezier(0.34, 1.56, 0.64, 1)`. Restart the isolated Vite server when switching. Final feel selection remains with the user.

Run from this Workspace repository root with existing Chromium and a compatible prebuilt backend:

```sh
LVK_TEST_SERVER=/absolute/path/to/compatible/server VITE_KANBAN_MOTION=crisp node tests/kanban/animation.mjs
LVK_TEST_SERVER=/absolute/path/to/compatible/server VITE_KANBAN_MOTION=bouncy node tests/kanban/animation.mjs
```

The dedicated test uses the production application through Vite, a new temporary SQLite instance, synthetic Issues and a linked Workspace, ephemeral loopback ports, and real API writes/readback. It does not mock API responses, copy user data, launch agents or expose a preview. `finally` closes Chromium/Vite/backend and deletes the test instance. `.build/kanban-animation-{crisp,bouncy}/` retains results, build identity, screenshots and a Playwright trace.

Coverage: priority-button drag prevention, actual sibling displacement and CSS timing at one/two grid columns (360/600px), overlay Web Animation keyframes/timing, pointer/keyboard reordering, focus restoration, Escape cancellation, populated/empty status moves, saved ordering/status after reload, click-to-detail. Geometry records any intersections with uneven-height cards using the unchanged rectSortingStrategy. Rendering/feel on a real smartphone and final human choice remain unverified. Cross-column sibling displacement is phase two and excluded.

## Integration boundary

Do not overwrite LOCAL-241's `useTouchClickGuard` import/call/handler spread when combining changes in `KanbanGrid.tsx`. LOCAL-237 changes only sortable motion options, wrapper transform/transition, and overlay settings; sensors, touch/click handlers, layout and commit processing stay unchanged. No new DnD dependency, main merge, deployment or preview2 change.

## LOCAL-240 dependency: existing rank and synchronization failures

User-confirmed ownership: LOCAL-240 has identified the real causes of fractional-rank HTTP 422, post-save synchronization reset and stale DnD source, and is implementing/verifying their fixes separately. LOCAL-237 deliberately does not duplicate those changes. The accepted scope here is limited animation E2E with integer fixtures and a reload between operations. Continuous operations must be tested again after integration with LOCAL-240.

The isolated current backend (`0a111959a108a8344c9a7604a44092a92707ac2d`, clean) rejects the normal browser request with a fractional midpoint, e.g. `{"updates":[{"id":"…","expected_sort_order":0,"sort_order":3.5}]}`, with HTTP 422: `updates[0]: invalid type: map, expected f64`. API/schema source has no difference from this Workspace base `a3247dad`. The test records a direct `.5` compatibility probe; it is not counted as successful persistence. Acceptance fixtures use ranks spaced by 1024 so tested midpoints remain integers, while still exercising real browser requests and API readback. This is a limitation of persistence validation, not a repair of fractional sorting. No backend or update-handler change is included under the agreed narrow scope.

## Verification results

- `pnpm run web-core:check`: PASS.
- `pnpm run local-web:check`: PASS.
- `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/kanban/ui/KanbanGrid.tsx --no-ignore --max-warnings 0`: PASS.
- `node --test tests/kanban/order.test.mjs`: 4/4 PASS (existing rank invariants).
- Scoped Prettier, `node --check tests/kanban/animation.mjs`, `git diff --check`: PASS.
- Both final candidate E2E commands: PASS for integer-rank fixtures; fractional-rank probe: HTTP 422 (known failure).

No Rust build or whole-application E2E suite was run. The two dedicated browser executions are real application tests with synthetic data, not mock-only component tests. A normal `.5` rank remains a known failing API case.

The existing fallback collection refreshes every 30 seconds. In exploratory runs, rapid consecutive gestures restored a stale snapshot after a saved reorder. The dedicated test checks API readback and reloads the board between independent gestures; it does not prove rapid repeated operations work. Focus is asserted before the reload, and cancellation is tested on a fresh persisted board. This synchronization issue is outside the animation-only change.

Exploratory attempts included an obsolete `aa59ab58` backend that showed no cards, the fractional 422 failure, a premature readback before async save, stale snapshot reset during consecutive gestures, and a test diagnostic callback rejected during cleanup. These are not counted as passes. The final harness records failures/traces and cleans up; all temporary instances from these attempts were removed after their processes had stopped.

Previous AgentRun `248e5ae0-c2ee-491a-ba21-6dc465d10ee4` started at 2026-10-27 14:47:22 UTC and failed at 14:47:27 UTC. MCP exposes no response body or error code for it, and the Workspace initially had no source changes. Its root cause remains unknown and is distinct from the validation failures above.

## Final browser evidence

- `crisp`: 2026-10-28 00:04:41–2026-10-28 00:05:42 JST, 60.9s elapsed including startup/waits. `.build/kanban-animation-crisp/results.json`, `revision.json`, `trace.zip`, `displacement-360.png`, `displacement-600.png`, `cross-column-1.png`, `cross-column-3.png`; console log `.build/kanban-animation-crisp.log`.
- `bouncy`: 2026-10-28 00:05:44–2026-10-28 00:06:45 JST, 60.8s elapsed including startup/waits. `.build/kanban-animation-bouncy/results.json`, `revision.json`, `trace.zip`, `displacement-360.png`, `displacement-600.png`, `cross-column-1.png`, `cross-column-3.png`; console log `.build/kanban-animation-bouncy.log`.

Final backend: `/var/tmp/lvk-instances/dev/releases/78979867-1a39-476c-9a93-fcc72586155d/server`, build revision `0a111959a108a8344c9a7604a44092a92707ac2d`, clean. It was read-only reused with a new temporary instance; no source differences in `crates/server`, `crates/api-types`, or `crates/db` from that revision to this task base. Source/test/backend SHA-256 values are in `.build/kanban-animation-tested-hashes.json`.

Both final runs measured one pair of overlapping non-dragged siblings at 360px while displaced; the 600px two-column fixture measured zero such pairs. Heights were unequal, dimensions were preserved without scaling, and screenshots capture the gaps/positions. This is not an assertion that every unequal-height arrangement is visually acceptable. The sorting strategy and layout remain unchanged; approval of that feel or a separate strategy change remains with the user. Human feel selection, real smartphone touch feel, production bundle/other browser engines and phase-two cross-column displacement are unverified.

All owned test instances/processes stopped and temporary DBs removed. About 22 MiB of local browser evidence remains intentionally; no duplicated backend build or new dependency was added. Final commit identity is recorded in `.build/kanban-animation-final-revision.json` after committing. Main remains at the task base; no merge, deployment or preview2 operation occurred.
