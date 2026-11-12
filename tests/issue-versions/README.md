# Issue version editor regression

From the repository root, after `pnpm install --frozen-lockfile` and installing Playwright Chromium:

```sh
node tests/issue-versions/browser.mjs
pnpm --filter @vibe/web-core exec vitest run src/pages/kanban/issue-version-drafts.test.ts src/shared/lib/electric/collections.test.ts
```

The browser fixture bundles the production `IssueVersionEditor` and Lexical editor. HTTP responses are mocked and no backend, shared database, agent, or existing development server is used. It checks tab isolation during a delayed save, saved status after persistence acknowledgement, 409 draft retention with version-specific errors, and external Issue revision notifications refreshing the saved current specification while preserving another version's unsaved draft.

Mobile light/dark screenshots are written to ignored `test-results/issue-versions/`. The runner closes Chromium in `finally` and bundles only in memory. These checks do not establish authenticated application/API integration or real multi-client server behavior.
