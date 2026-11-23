# Isolated saved-filter acceptance

Run only against an explicitly supplied disposable integration instance:

```sh
LOCAL460_HANDOFF=.lvk/shared/tmp/local460-integration/handoff.json \
  pnpm exec playwright test --config tests/saved-filters/runtime/playwright.config.ts
```

The suite reads the handoff locally, rejects shared dev port 4020, and never
prints authentication fields. It uses the machine Project and adds identifiable
fixture Issues/filters through the real HTTP API. Existing fixture priority
changes are restored after bulk assertions. Do not run concurrently with another
writer of the same fixtures or the user's board preferences.

Natural-language text is saved and edited, then cleared before selecting its
filter. Evaluate requests are guarded against natural-language definitions and
the semantic Project. These tests do not exercise a provider, and do not change
or stop the supplied runtime. The browser timing/failure case deliberately
injects network delay/abort; its error states are not backend semantic failures.

The local Project snapshot poll is 30 seconds. Assertions allow that refresh;
they do not force a reload to simulate live updates. Test artifacts and screenshots
are written under `.build/saved-filters/runtime/`. The ordinary component fixture
suite and its Vite server are independent of this opt-in suite.

Cases cover real settings CRUD and stable links, residual-filter overrides,
selection pruning and reversible priority updates in both board layouts, live
Issue/definition changes and relative expiry, ordinary versus saved search,
and all editor fields. Bulk updates are observed as the union of actual HTTP
writes (currently one PATCH per Issue), then checked against server state.

The temporary-filter override case also exercises nested-menu Escape dismissal.
It requires a frontend containing the nested-menu modal fix; an older runtime
can retain `body { pointer-events: none }` before a saved filter is selected.
Record that failure separately from selection/bulk and from the component
regression in `../temporary-filters.spec.ts`. Keep the runtime's source revision
and served asset hashes with results; the source checkout HEAD alone does not
identify the tested frontend.

The suite leaves its additional fixture Issues/filters for inspection in the
disposable instance. It restores priority changes to existing fixtures in
`finally`; it does not delete Issues or clean up another writer's fixtures.
