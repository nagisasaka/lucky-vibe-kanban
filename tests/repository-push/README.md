# Repository publication fixture

Run from the repository root after installing dependencies and Playwright Chromium:

```sh
node tests/repository-push/browser.mjs
pnpm --dir packages/web-core exec vitest run src/shared/dialogs/settings/settings/RepositoryPushSettings.test.tsx
node scripts/dev-cargo.mjs test -p git repository_push
node scripts/dev-cargo.mjs test -p server repository_push
```

The browser bundles the product settings component and shipped CSS with mock client actions. Every browser request is intercepted. It verifies explicit source/remote/branch/archive review and confirmation, no-admission recovery, lost-response locking and reload/read-only verification, plus narrow dark and desktop light layouts. Screenshots go to ignored `test-results/repository-push/`.

Git unit tests use task-owned temporary local bare remotes and rejection hooks. Server tests use migrated in-memory databases, authenticated fixture actors and seeded durable receipts. They cover authority, target validation before persistence, exact-input replay and restart uncertainty. These fixtures do not establish real GitHub credential validity, GitHub branch rules, live OAuth/MCP interoperability or authenticated full-application behavior.
