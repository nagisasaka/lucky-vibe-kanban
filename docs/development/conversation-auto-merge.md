# Conversation-owned Auto-merge

New integrations start their assigned Codex immediately in one interactive Workspace and Session. The bundled [auto-merge skill](../../assets/skills/auto-merge/SKILL.md) owns merge preparation, conflict repair, risk-based verification and enabled OpenWiki maintenance. The host no longer chooses the next source or requires a resolution-review gate for these jobs. Retained mechanical integrations keep their existing reader and recovery path; the `conversational` payload flag prevents replaying their history as a new conversation.

The upstream MCP client selects immutable source commits and the admitted target SHA with `start_integration`. Optional `instructions` carries partial validation omissions or other user requirements. Existing `steer_execution`, `continue_execution`, questions and bounded reads operate on the assigned Session. Neither upstream instructions nor a review comment grant new publication scope.

The local worker has one context-bound stdio operation, `integration_action`:

- `inspect` reads the frozen selection and retained evidence.
- `verify` starts an existing managed ScriptRequest on an expected clean code SHA. The process ID, command result and before/after SHA remain associated with that code commit.
- `skip` records a user/agent decision, reason and remaining risk. It does not manufacture a passed process.
- `publish` records the code commit, optional Wiki commit and unchanged final candidate, then returns. Acceptance is not publication. The worker must finish; the host waits for the Run, process registry and scripts to settle before invoking exact Git promotion.

Request IDs bind identical inputs. A repeated accepted verification does not launch another process; its process record remains inspectable after a disconnected caller. A replacement candidate receives new verification/skip records. Old failures remain history. A completed code test is never labelled as a test on the subsequent Wiki commit.

When Wiki is enabled, the same Codex uses the existing OpenWiki lifecycle after code verification, makes Wiki-only changes in a separate commit and submits the combined candidate. The host reuses the existing Native Audit completion proof. The publication input binding preserves the recorded code tree across the Wiki-only commit; a repair cannot be presented as a Wiki continuation of an older tested code SHA. This runs inside publication input checks, without a separate phase or repeated code tests. There is no extra Wiki/code-check phase or post-publication Wiki system Workspace. Standalone initialization and synchronization remain independent. Published selected event receipts and freshness are recorded through the existing memory store, without a new integration outbox requiring a successor agent.

The ordinary conversation displays host-confirmed candidate, verification/skip and publication facts. Model prose does not trigger success. Publication intent, confirmed application and a stopped unpublished candidate remain different facts. Successful generated Issues and Workspaces use the existing Done/archive lifecycle without a Codex restart.

Target movement does not interrupt candidate work. At publication the host may promote the same candidate only if it contains the actual target; the exact ref transaction compares that observed target again. No merge/rebase or tracking loop is introduced. Otherwise the job stops unpublished in review, retains its candidate/evidence/repair context, and releases execution reservations after settlement. A new instruction can continue the same conversation or select retained work into a new integration. Git-applied publication with an uncertain DB receipt is reconciled through the existing durable intent; it is never converted to an unpublished stop or undone.

## Verification

Use the existing crate fixtures for candidate evidence, owner binding, cancellation, exact publication and OpenWiki audit contracts. The UI fixture renders host facts without parsing model text:

```sh
node scripts/dev-cargo.mjs test -p server --lib conversation_
node scripts/dev-cargo.mjs test -p db conversation_tests
node scripts/dev-cargo.mjs test -p mcp
pnpm --dir packages/web-core exec vitest run src/features/workspace-chat/ui/IntegrationFacts.test.tsx
```

The ignored `isolated_conversation_repair_publication_drift_and_idempotence` fixture requires a fresh task-owned `LVK_INSTANCE_DIR` whose basename starts with `lvk257-test-`. It reserves a deterministic provider identity without launching a model, and exercises real isolated Git, SQLite and managed commands. It must run alone in its process; never supply a running instance directory. Public integrations and standalone Wiki generation are not needed for these tests.
