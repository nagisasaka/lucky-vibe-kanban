# Change Manifest: Codex allowance beside server resources

Workspace: `c4e61a99-d2d8-4c2a-9822-40604d3c7ba0`

TODO: `5f2c5d61-4225-4d2f-bd9c-f2040d759892`

Date: 2026-10-29

## Product changes

- Shared PC/mobile Navbar resource slot now includes a compact Codex remaining
  allowance button. Radix popover supports click/tap, Enter and Escape/focus return.
- Default-bucket window minimum is the summary. Missing default means unknown.
  Same public limit IDs are deduplicated; additional buckets have safe public
  name/ID labels. Independent buckets and credits are never combined.
- Real API durations use exact minute/hour/day units. Unknown, invalid, stale,
  unsupported, failed and reset-elapsed values display `—`, not fabricated 0/100%.
- Observation/reset times and semantics appear in details. Existing English and
  Japanese common i18n resources cover all new product text.
- Dedicated shared query uses five-minute visible polling, ten-minute freshness,
  no automatic retries/focus refetch, and a thirty-second manual cooldown after
  success or failure. Resource seconds polling starts no Codex processes.

## Boundaries and source review

Only frontend/product tests and documentation change. Backend authentication,
authorisation, transport, provider launch and cleanup remain existing contracts.
The [test guide](README.md#source-contract-review-2026-10-29) records the source
checks for `ApiResponse.data.result.quota`, local transport, CSRF, Origin and
administrator/read-scope enforcement. No account identifiers/raw reasons are
rendered or logged. No model inference was invoked.

Independent review corrections are included: no arbitrary representative,
limit-ID deduplication, safe public labels, real readable periods, en/ja i18n,
and additional model/browser assertions.

## Validation

- `node --test tests/codex-quota/model.test.mjs tests/resource-observation/model.test.mjs`: 11 passed.
- `pnpm run ui:check && pnpm run web-core:check && pnpm run local-web:check`: passed.
- Scoped UI ESLint and web-core ESLint with local-web's explicit config: passed.
- `node tests/resource-observation/browser.mjs`: 15 regression cases passed.
- `node tests/codex-quota/browser.mjs`: 15 cases passed; final evidence in ignored `test-results/codex-quota/results.json`.
- Scoped Prettier and `git diff --check`: passed.

Development checks initially caught a TypeScript inferred-union error and an
intentional control-character regex lint requirement; both were corrected.
Expanded browser-clock tests caught fixture time-domain and response-settlement
races; the fixture uses browser timestamps and waits for completed refreshes.
A desktop nowrap experiment also exposed control/title overlap at 768px; the final layout wraps at constrained widths and keeps a single row at 1280px, with control-overlap assertions. These failed iterations are not counted as successful runs.

## Review limits and evidence

No live authenticated server/official Codex/account call, actual physical phone,
backend Rust build, main integration, restart or publication. Fixture E2E is not
live API interoperability. User explicitly deferred heavy builds/integration.

Ignored screenshots/results: `test-results/codex-quota/` and the existing
resource regression evidence directory. Disposable browser/server/bundles are
stopped/removed; installed dependencies are retained for workspace development.
No investigation logs or screenshots are committed. Final source SHA is supplied
in the completion report. Candidate awaits upper AI review.
