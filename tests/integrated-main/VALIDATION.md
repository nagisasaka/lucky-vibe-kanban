# LOCAL-65: fixed integrated-main validation

Base: `1b3deac8276f3165c483a30ac04aff2c9555cff1` (2026-10-21).
Workspace: `aa126589-3025-4335-b504-cb7d636c17f0`, branch `vk/aa12-19-main-e2e`.
The final validation revision is the commit containing this report; the handoff records its full SHA. This task performed no main tracking, direct merge, deployment, Docker build/restart, live database repair or authentication change. The user separately rebooted the VPS/container after apt/kernel updates.

## Evidence and execution

Workspace-local retained evidence is `.build/integrated-main-validation/` (`E` below). `commands.jsonl` records exact argv, base HEAD, dirty patch digest, total exit, duration and log path for each invocation. Initial failures are retained separately from reruns. `build-artifacts.json` (initial) and `build-artifacts-final.json` (rebuild) identify the built server, agent-process-host, MCP binary and frontend assets by SHA-256. These were built from the fixed base plus the test-only Rust fixture initialization; the initial-highlight fix was separately rebuilt and its affected native/browser paths passed, recorded below. Build metadata correctly reports a dirty source tree.

`ancestry.json` verifies all 19 source commits are ancestors of the base. `mechanical-merge-checks.json` checks buildInfo/header integrations, both web-push/libc dependencies, all fake_introspection permission branches, both locale key groups and TodoPanel imports. `static-less.json` records Dockerfile installation and smoke definitions only.

Heavy jobs ran sequentially, Cargo/Rust test workers at most 2, browser/Vitest workers 1. Tests use owned instance directories, SQLite databases, repositories and loopback ports. The resource snapshot had no registered resources, holders or queue. The resumed task's old PID was absent and no prior task descendants were found; this does **not** certify settlement of the entire LVK registry. Provider-capacity/database-locked failures of the earlier agent runs were not repaired or counted as product validation.

## Minimal corrections

One production defect was reproduced independently in the native application and a component regression: initially loaded code blocks stayed plain until a subsequent Markdown update. Register code transforms in a layout effect before the initial Markdown synchronization; streaming, unknown/huge fallback and whitespace behavior remain covered. The initial component regression failed before this change and passed after it.

- `run_events.rs`: initialize `Remote.environment` to `None` in two test fixtures. Initial broad lint failed with E0063; corrected lint and the complete backend test command passed.
- Canonical conversation unit: retain message order/identity assertions and additionally assert the fourth, synthetic elapsed footer's key and timing. A derived footer has no provider-event identity. Initial result was 536 passed / 1 failed; rerun was 537 passed across 90 files.
- TODO browser test: use the integrated Japanese 自動マージ heading and verify opening/closing. Preserve both the initial English-selector failure and the unsuccessful intermediate dialog-role assumption.
- Input browser test: pending send permits editing the next draft. The separate repeated-Enter single-submission assertion remains intact.
- Chat browser test: Ctrl+Enter inserts a newline, then Enter sends. Assert the exact multiline submitted prompt and that Ctrl+Enter did not send. The older test expected Ctrl+Enter to submit.

No assertion was removed, no skip was added, and no product requirement was relaxed to obtain a pass.

## Interpretation and limits

Native file-link tests use the built application, standard file pane, real file API and actual selected-line scrolling. Conversation history is seeded into a disposable DB. The separate file-link component fixture injects a conversation layout and is not a native application E2E.

Syntax/scroll tests use synthetic provider events. Input tests deliberately mock selected follow-up responses while retaining real Scratch persistence; IME events are synthetic. Elapsed tests seed canonical run/event records. Terminal and approval tests use a local provider protocol fixture with the actual process host, event persistence and browser/API. These do not demonstrate real Codex/model execution. Reasoning tests mock discovery/follow-up. External GitHub responses are mocked to distinguish 0 stars from failure. Mobile coverage is Chromium viewport emulation, not physical devices.

Server-kit tests skip unavailable Docker Compose and nginx checks; reasoning HTTPS ingress is skipped. Auth0 real authentication, real OS IME, physical devices, real models, stable Docker image/less execution and live DB migration are not validated. The historical LOCAL-11 migration `20261020190000` collision in an already-running database is explicitly outside this isolated test: current LOCAL-19 is `20261020190000`, current LOCAL-11 is `20261020200000`.

The earlier backend-test log has 46 successful-looking summaries but no overall exit and remains unconfirmed; the separately recorded complete rerun is the evidence of success. Root-level Babel resolution failed for the existing MCP-links test. A temporary symlink to the already-installed local-web plugin allowed that test to pass and was removed; this is an environment workaround, not an unmodified-root pass.

## Handoff condition

Review the minimal highlighting fix and test corrections before formal integration. Revalidate the formally merged HEAD (including build/check/lint/backend tests and affected browser boundaries) before declaring the task complete. This workspace's results do not certify any later main SHA. Deployment requires separate authorization.

## Supplemental interaction evidence

- Fresh and representative upgrade databases (before LOCAL-19/11, and after current LOCAL-19): all 110 migration versions/checksums, foreign keys, integrity, status IDs and retained issue data passed. Temporary databases were removed after server shutdown. No production DB was read.
- Initial highlight regression: `chat-native-interaction.log`, diagnostic DOM log and JSON-language rerun show plain initial code. `syntax-initial-regression.log` independently reproduces failure. `syntax-initial-layout-fix.log` passes after the layout-effect correction. Native rebuilt interaction passes in `chat-native-interaction-fixed.attempt2.log` (screenshot `interaction.png`).
- The supplemental test initially demanded persisted file-line selection after reload. `useWorkspaceFilePreviewState` intentionally holds this in component state. Corrected supplemental scope checks restored draft/elapsed/highlight and reopens the actual file link at line 120; the original failure remains in `chat-native-interaction-fixed.log`. This was not an existing acceptance assertion or a product change.
- The steering protocol fixture initially echoed Markdown-escaped underscores instead of interpreting the requested token. Its first failure is retained; the fixture now returns the unescaped token while unchanged API/event exactly-once assertions verify persistence and reconnect. This does not establish real-model behavior despite the inherited test title saying “real Codex”.
- Header and Wiki supplemental viewport is actually 320×844. The copied header runner's result label still says 390×844; its recorded source and screenshot establish the actual 320px viewport.

- Approval restart + Kanban: the first supplemental attempt used an unsupported single-issue GET endpoint (405); no product change. The corrected public fallback-list endpoint test passed. Two linked executions stayed `awaiting_approval` and the card stayed In Progress across an owned-server restart with zero implicit native decisions. Approving/completing the newer run kept In Progress while the older run remained pending; cancelling the older run moved the card to In Review. Exactly one native approval decision occurred, and cleanup found no owned processes.

## Coverage by integrated feature

| Features | Evidence |
| --- | --- |
| 21/26/29/30/32 chat | Full chat browser suite: recent-two load, prepend/anchor/late workspace response, keyboard/IME/failure/typeahead/Scratch; process-host steering persisted once through reconnect/reload. Syntax native initial-history regression, stream/copy/huge/unknown tests; fixed elapsed and actual file API/line 120 in the same view. Path and symlink boundaries covered by file-link API tests. |
| 25/26 TODO | Built browser + real backend/MCP CRUD, validation/stale revisions, 320–1600px layout/reading/Tab order; existing hooks/import static check. |
| 31/33/23/34 Workspace/Wiki/header/buildinfo | Existing real-server navigation and Wiki suites, saved pane/draft/back/reload, 320px supplemental header/Wiki, build metadata from actual executable in unrelated launch cwd, keyboard/focus/tap/close. External GitHub 0/error scenarios mocked. |
| 16/37/11 MCP | Complete Rust suite: bounded code reads/search/paths/batches, schemas/catalogues, permissions/self-grant exclusion, runtime-access persistence/restart/recovery/corruption and process lifecycle, observation unknown/partial/timeout/semaphore limits. A separately observed live resource-cancellation E2E is not claimed. No live permission changes or live observation smoke certification. |
| 27/28/29/38 + 19 executor/Kanban | Reasoning UI/Scratch and transport/unit coverage; five terminal latency/reconnect/reload cases; supervised command/file/parallel allow/deny/cancel/unknown/late duplicate cases with native audit verification. Explicit Never transport tests and full-access browser case. Two-run approval restart/Kanban interaction plus DB latest-actual-start/workspace/order tests. |
| 22 conflict review | Isolated real API/Git browser scenarios approve/cancel/stale/empty explanation, keyboard details, reload and publication exactly once; publication-intent ambiguity covered in Rust suite. |
| 24 less | Dockerfile dependency and smoke definition inspected; no Docker/image execution, as required. |

## Artifact retention

Evidence scripts/logs/screenshots and command receipts remain under `E`, with existing runner evidence under `.build/` and `test-results/`. Owned temporary instance state is removed after process settlement; no unrelated workspace is cleaned. The normal workspace `target/` (~49 GiB), installed dependencies and current frontend `dist/` are retained for review/retest in accordance with AGENTS.md. They are not duplicated as evidence. Shared sccache's actual directory differs from the requested shared cache path; it was observed and not restarted/reconfigured.

## Per-command results

All exits below are total command exits; 0 does not turn internal skips into passes. Commands run from the repository root through `E/run.py` (environment/ownership wrapper). Relative log links point to retained workspace evidence.

| Invocation | Exact command | Exit | Log |
| --- | --- | ---: | --- |
| frontend-build | `pnpm --dir packages/local-web run build` | 0 | [frontend-build.log](../../.build/integrated-main-validation/frontend-build.log) |
| check | `pnpm run check` | 0 | [check.log](../../.build/integrated-main-validation/check.log) |
| lint | `pnpm run lint` | 101 | [lint.log](../../.build/integrated-main-validation/lint.log) |
| lint-after-fixture-fix | `pnpm run lint` | 0 | [lint-after-fixture-fix.log](../../.build/integrated-main-validation/lint-after-fixture-fix.log) |
| backend-test-resume | `pnpm run backend:test` | 0 | [backend-test-resume.log](../../.build/integrated-main-validation/backend-test-resume.log) |
| backend-build | `cargo build --locked --bin server --bin agent-process-host --bin vibe-kanban-mcp` | 0 | [backend-build.log](../../.build/integrated-main-validation/backend-build.log) |
| web-core-unit | `pnpm --dir packages/web-core exec vitest run --maxWorkers=1` | 1 | [web-core-unit.log](../../.build/integrated-main-validation/web-core-unit.log) |
| web-core-unit-after-footer-fix | `pnpm --dir packages/web-core exec vitest run --maxWorkers=1` | 0 | [web-core-unit-after-footer-fix.log](../../.build/integrated-main-validation/web-core-unit-after-footer-fix.log) |
| node-component | `node --test --test-concurrency=1 tests/chat/conversation-layout.test.mjs tests/chat/conversation-sync.test.mjs tests/chat/editor-display.test.mjs tests/chat/executor-config.test.mjs tests/chat/run-elapsed.test.mjs tests/chat/syntax-highlight.test.mjs tests/build-info/component.test.mjs tests/mobile-preview/preview.test.mjs tests/mobile-preview/settings.test.mjs tests/web-push/service-worker.test.mjs tests/server/mcp-links.test.mjs` | 1 | [node-component.log](../../.build/integrated-main-validation/node-component.log) |
| mcp-links-with-package-resolution | `python3 .build/integrated-main-validation/mcp-links-env.py` | 0 | [mcp-links-with-package-resolution.log](../../.build/integrated-main-validation/mcp-links-with-package-resolution.log) |
| server-kit | `pnpm run server:check` | 0 | [server-kit.log](../../.build/integrated-main-validation/server-kit.log) |
| header-e2e | `node tests/header/isolated.mjs` | 0 | [header-e2e.log](../../.build/integrated-main-validation/header-e2e.log) |
| build-info-e2e | `node tests/build-info/isolated.mjs` | 0 | [build-info-e2e.log](../../.build/integrated-main-validation/build-info-e2e.log) |
| todos-e2e | `node tests/todos/isolated.mjs` | 1 | [todos-e2e.log](../../.build/integrated-main-validation/todos-e2e.log) |
| todos-e2e-after-localized-title | `node tests/todos/isolated.mjs` | 1 | [todos-e2e-after-localized-title.log](../../.build/integrated-main-validation/todos-e2e-after-localized-title.log) |
| todos-e2e-after-heading-fix | `node tests/todos/isolated.mjs` | 0 | [todos-e2e-after-heading-fix.log](../../.build/integrated-main-validation/todos-e2e-after-heading-fix.log) |
| input-e2e | `node tests/chat/input-operations.isolated.mjs` | 1 | [input-e2e.log](../../.build/integrated-main-validation/input-e2e.log) |
| input-e2e-typeahead | `node tests/chat/input-operations.isolated.mjs` | 0 | [input-e2e-typeahead.log](../../.build/integrated-main-validation/input-e2e-typeahead.log) |
| file-links-native | `node tests/chat/file-links.e2e.mjs --native` | 0 | [file-links-native.log](../../.build/integrated-main-validation/file-links-native.log) |
| file-links-component | `node tests/chat/file-links.e2e.mjs` | 0 | [file-links-component.log](../../.build/integrated-main-validation/file-links-component.log) |
| syntax-e2e | `node tests/chat/syntax-isolated-e2e.mjs` | 0 | [syntax-e2e.log](../../.build/integrated-main-validation/syntax-e2e.log) |
| elapsed-e2e | `node tests/run-elapsed/isolated.mjs` | 0 | [elapsed-e2e.log](../../.build/integrated-main-validation/elapsed-e2e.log) |
| workspace-navigation-e2e | `python3 tests/workspace-navigation/run-isolated.py` | 0 | [workspace-navigation-e2e.log](../../.build/integrated-main-validation/workspace-navigation-e2e.log) |
| workspace-wiki-e2e | `node tests/server-access/workspace-wiki.mjs .build/integrated-main-validation/wiki` | 0 | [workspace-wiki-e2e.log](../../.build/integrated-main-validation/workspace-wiki-e2e.log) |
| reasoning-e2e | `node tests/server/run-reasoning-override.mjs` | 0 | [reasoning-e2e.log](../../.build/integrated-main-validation/reasoning-e2e.log) |
| terminal-e2e | `python3 tests/terminal-latency/run.py` | 0 | [terminal-e2e.log](../../.build/integrated-main-validation/terminal-e2e.log) |
| supervised-built-e2e | `python3 .build/integrated-main-validation/supervised-built.py` | 0 | [supervised-built-e2e.log](../../.build/integrated-main-validation/supervised-built-e2e.log) |
| integration-review-e2e | `python3 tests/integration-review/run.py` | 0 | [integration-review-e2e.log](../../.build/integrated-main-validation/integration-review-e2e.log) |
| migrations | `python3 .build/integrated-main-validation/migrations.py` | 0 | [migrations.log](../../.build/integrated-main-validation/migrations.log) |
| chat-native-interaction | `node .build/integrated-main-validation/interaction.mjs --native` | 1 | [chat-native-interaction.log](../../.build/integrated-main-validation/chat-native-interaction.log) |
| chat-native-interaction-diagnosis | `node .build/integrated-main-validation/interaction.mjs --native` | 1 | [chat-native-interaction-diagnosis.log](../../.build/integrated-main-validation/chat-native-interaction-diagnosis.log) |
| chat-native-interaction | `node .build/integrated-main-validation/interaction.mjs --native` | 1 | [chat-native-interaction.attempt2.log](../../.build/integrated-main-validation/chat-native-interaction.attempt2.log) |
| header-320 | `node .build/integrated-main-validation/header-320.mjs` | 0 | [header-320.log](../../.build/integrated-main-validation/header-320.log) |
| wiki-320 | `node .build/integrated-main-validation/wiki-320.mjs .build/integrated-main-validation/wiki-320` | 0 | [wiki-320.log](../../.build/integrated-main-validation/wiki-320.log) |
| chat-provider-fixture | `python3 .build/integrated-main-validation/chat-fixture-run.py` | 1 | [chat-provider-fixture.log](../../.build/integrated-main-validation/chat-provider-fixture.log) |
| syntax-initial-regression | `node --test tests/chat/syntax-highlight.test.mjs` | 1 | [syntax-initial-regression.log](../../.build/integrated-main-validation/syntax-initial-regression.log) |
| syntax-initial-layout-fix | `node --test tests/chat/syntax-highlight.test.mjs` | 0 | [syntax-initial-layout-fix.log](../../.build/integrated-main-validation/syntax-initial-layout-fix.log) |
| frontend-build-after-initial-highlight | `pnpm --dir packages/local-web run build` | 0 | [frontend-build-after-initial-highlight.log](../../.build/integrated-main-validation/frontend-build-after-initial-highlight.log) |
| ui-check-final | `pnpm run ui:check` | 0 | [ui-check-final.log](../../.build/integrated-main-validation/ui-check-final.log) |
| web-core-check-final | `pnpm run web-core:check` | 0 | [web-core-check-final.log](../../.build/integrated-main-validation/web-core-check-final.log) |
| ui-lint-final | `pnpm run ui:lint` | 0 | [ui-lint-final.log](../../.build/integrated-main-validation/ui-lint-final.log) |
| chat-native-interaction-fixed | `node .build/integrated-main-validation/interaction.mjs --native` | 1 | [chat-native-interaction-fixed.log](../../.build/integrated-main-validation/chat-native-interaction-fixed.log) |
| chat-native-interaction-fixed | `node .build/integrated-main-validation/interaction.mjs --native` | 0 | [chat-native-interaction-fixed.attempt2.log](../../.build/integrated-main-validation/chat-native-interaction-fixed.attempt2.log) |
| syntax-built-final | `node tests/chat/syntax-isolated-e2e.mjs` | 0 | [syntax-built-final.log](../../.build/integrated-main-validation/syntax-built-final.log) |
| chat-provider-fixture-final | `python3 .build/integrated-main-validation/chat-fixture-run.py` | 0 | [chat-provider-fixture-final.log](../../.build/integrated-main-validation/chat-provider-fixture-final.log) |
| approval-restart | `python3 .build/integrated-main-validation/approval-restart.py` | 1 | [approval-restart.log](../../.build/integrated-main-validation/approval-restart.log) |
| approval-restart | `python3 .build/integrated-main-validation/approval-restart.py` | 0 | [approval-restart.attempt2.log](../../.build/integrated-main-validation/approval-restart.attempt2.log) |
| input-final | `node tests/chat/input-operations.isolated.mjs` | 0 | [input-final.log](../../.build/integrated-main-validation/input-final.log) |
| todos-final | `node tests/todos/isolated.mjs` | 0 | [todos-final.log](../../.build/integrated-main-validation/todos-final.log) |
| wiki-final | `node tests/server-access/workspace-wiki.mjs .build/integrated-main-validation/wiki-final` | 0 | [wiki-final.log](../../.build/integrated-main-validation/wiki-final.log) |
| chat-component-final | `node --test --test-concurrency=1 tests/chat/conversation-layout.test.mjs tests/chat/conversation-sync.test.mjs tests/chat/editor-display.test.mjs tests/chat/executor-config.test.mjs tests/chat/run-elapsed.test.mjs tests/chat/syntax-highlight.test.mjs` | 0 | [chat-component-final.log](../../.build/integrated-main-validation/chat-component-final.log) |
| final-diff-check | `git diff --check` | 0 | [final-diff-check.log](../../.build/integrated-main-validation/final-diff-check.log) |

The initial interrupted `pnpm run backend:test` has **no recorded total exit** (`E/backend-test.log`) and is not counted as passed. The complete `backend-test-resume` rerun exits 0 with 46 successful test-summary lines and existing ignored fixtures/live-only tests. Full web-core unit rerun: 90 files / 537 tests passed. Server-kit: 29 passed / 7 skipped. Reasoning browser: 2 passed / 1 skipped (HTTPS ingress). No new skip was introduced.

## Recovery after user-operated VPS/container reboot

The prior AgentRun `9ba7cdd6-3d84-4463-9bbd-47ecfd1ab49f` was reconciled as crashed after the user-operated restart; MCP was reported restored at 08:51 UTC. This interruption happened after the final product regression receipts had been saved but before commit/handoff. The older memory note that rebuild/native retest remained unfinished was stale. No model or operational authentication setting was changed during recovery.

`post-reboot-audit.json` confirms the fixed HEAD, identical tested source patch (`3c40256122eca22df06ea472da10b586bac64b8601909ff1bd12bd7aa88a519d`), all 52 prior command logs present, no owned test processes/listeners/temporary instances, and empty shared-resource holders/queue. This is not a claim of whole-registry settlement. All 991 rebuilt frontend files match their recorded hashes (`post-reboot-artifact-check.json`).

A single post-reboot native interaction rerun verifies that the retained built server and Chromium still work: initial syntax highlight, unknown-language plain text, fixed elapsed footer, real file API/line 120, reload and draft persistence. It exits 0 and records no remaining owned processes after cleanup. Already-confirmed broad tests were not repeated merely because the agent run crashed.

Command: `node .build/integrated-main-validation/interaction.mjs --native`; total exit 0; [post-reboot log](../../.build/integrated-main-validation/post-reboot-native-interaction.log).
