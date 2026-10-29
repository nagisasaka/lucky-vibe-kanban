# Codex allowance beside server resources

The Navbar uses the existing authenticated `POST /api/provider-usage/read`
with `provider=codex`, `account_context=server_default`, `include_usage=false`.
It creates no model turn. Resource polling remains independent.

The summary uses only the API-designated default bucket and the minimum of
its reported window remaining percentages. Without a default bucket the
summary is unknown; independent additional buckets remain visible in details.
Duplicate `limitId` entries are omitted, preferring the default bucket. Public
`limitName`/`limitId` labels are escaped by React, stripped of control/bidi
characters and bounded to 120 characters. Account identifiers are not projected.
Durations come from the response (300 minutes → 5 hours, 10080 → 7 days).
English/Japanese strings use the existing `common` i18n namespace.

Used percentages must be finite and in [0, 100]. Missing, failed, unsupported,
stale (10 minutes), future observations and elapsed resets never imply full or
zero allowance. Credits/unlimited are separate from ordinary quota. Details
show observation/reset times. No Workspace/context tokens are inferred.

React Query shares in-flight/cache data across subscribers. Automatic reads run
at five-minute intervals while visible. Manual reads have a 30-second cooldown
following success or failure. Hidden documents do not start interval reads;
an already-started passive read retains the backend's existing cleanup lifecycle.

## Checks

From the repository root:

```sh
node --test tests/codex-quota/model.test.mjs tests/resource-observation/model.test.mjs
node tests/codex-quota/browser.mjs
node tests/resource-observation/browser.mjs
pnpm run ui:check && pnpm run web-core:check && pnpm run local-web:check
pnpm --dir packages/ui exec eslint src/components/CodexQuota.tsx src/lib/codexQuota.ts --max-warnings 0
pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/shared/hooks/useCodexQuota.ts ../web-core/src/shared/hooks/useResourceObservation.ts ../web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx --max-warnings 0
```

The Chromium fixture exercises real Navbar, presentation, hook and CSS with
isolated API responses at 320/390/768/1280px, keyboard and click details,
errors/unsupported/stale/reset, public bucket names, duplicates, no-default,
English/Japanese and polling/cooldown/visibility. It does not contact a running
LVK server, official CLI or account. Screenshots/results stay Git-ignored in
`test-results/codex-quota/`. Scratch bundles/server/browser are cleaned up.

## Source contract review (2026-10-29)

- `crates/server/src/routes/provider_usage.rs` returns `ApiResponse::success(value)`.
  `crates/server/src/provider_usage.rs` wraps the passive result under `result`;
  `crates/executors/src/executors/codex/account_read.rs` returns `quota` with
  `status`, `observedAt`, `data`. Thus the UI consumes
  `ApiResponse.data.result.quota`, with camelCase window/bucket fields.
- `makeLocalApiRequest` in `localApiTransport.ts` uses the current local host
  and a relative API URL; remote host IDs are rejected. Bootstrap installs
  `installAccessFetch`, which adds `x-lvk-csrf` only to same-origin API mutations.
- `routes/mod.rs` merges the existing provider route, validates Origin and wraps
  it with server-access middleware. That middleware authenticates the web
  principal and validates CSRF for POST; the provider route additionally requires
  administrator/internal capability and `lvk:read`. No permission was broadened.
  Denied reads are shown as unavailable without exposing response bodies.

These are source checks, separate from fixture E2E. Live authenticated API/CLI,
real account interoperability, physical phone browsers, deployment and backend
builds are unverified. No main merge, restart or publication is part of this change.
