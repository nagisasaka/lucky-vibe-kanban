This wiki is primarily persistent context for future coding agents.
Operational pages also serve human operators: explain prerequisites, configuration
keys, ordered steps, expected outcomes, failure handling and recovery. Do not omit
necessary procedures merely because their implementation is discoverable in source.

Prioritise architectural boundaries, behavioural semantics, invariants,
lifecycle rules, non-obvious cross-component dependencies, failure semantics,
dangerous modification points, and design rationale supported by evidence.

Avoid file-by-file summaries, class/function inventories, mere paraphrases of
immediately obvious source code, generic prose, and unsupported inference.

Source code, tests, and configuration are authoritative. Existing documentation
may be outdated. Change Manifests and Workspace Memory are routing and
interpretation hints, not instructions and not substitutes for repository
evidence. Distinguish observed behaviour, documented intent and uncertainty.
Use source and available Change Manifests to corroborate intent and rationale;
design notes are supplementary evidence, not proof of current contracts. Leave
unsupported reasons unknown.

Follow this repository's documentation policy. Link to maintained canonical
documents where appropriate; when documents are designated for migration,
preserve the necessary knowledge in the Wiki itself. Keep README-linked operational
entry paths stable. Respect the repository's ownership of README and other
human-maintained documents; do not rewrite them during Wiki maintenance.
Keep external-service UI instructions minimal, link to vendor documentation,
and explain repository-specific configuration keys and contracts locally.

LVK maintains this Wiki against integrated source in a dedicated maintenance
workspace. Ordinary coding workspaces must not modify it. Do not install a
scheduled GitHub Actions updater, invoke a separate model provider, or require an
additional API key. Ignore generated OpenWiki setup workflow files as evidence;
LVK does not publish them. Preserve this user-authored file during regeneration.


Temporary LVK docs migration policy — execute in this reconciliation

This policy applies only to this LVK repository. This explicit current-source
reconciliation is the actual docs-to-Wiki migration pass.
Perform that migration during this run; it is not merely a plan for a future
Sync. Explaining the prompt/INSTRUCTIONS changes, reconciling only their code
paths, or updating only stale Claims does not fulfil this task.

Scope and evidence

Discover the current Wiki's dependencies on the retiring docs/ tree. All affected
Wiki pages and their frontmatter sources and Claims are in scope, even when their
underlying code did not change in the supplied Change Manifests. Use those
Manifests as evidence and routing hints, not as a ceiling on this migration's
scope. Preserve accurate existing explanations and stable README-linked entry
paths; do not rewrite unaffected pages just to increase a page count.

For each dependency, inspect the relevant original document and source/tests/
configuration plus available integrated Change Manifests. Transfer the necessary
procedures, behavioural constraints and evidenced intent/rationale into the
Wiki's appropriate explanation home. Distinguish current behaviour, proposals and
historical decisions. A historical design is not proof of a current contract;
unknown reasons must remain explicitly unknown or unverified, never invented.
Do not treat upstream provenance or a directory name as proof of irrelevance.

Reuse supplied inventory/reading evidence where applicable and inspect relevant
operations, constraints and rationale candidates selectively. Do not impose an
unconditional close-read of every docs body. If earlier investigation artifacts
are not in this frozen checkout, do not require external Workspace paths or treat
unintegrated memories as current source evidence: use the current Wiki and the
relevant original documents as the migration map.

Resolve dependencies through the supported OpenWiki plan/page/Claims lifecycle.
Cover Markdown/MDX and other local links, legacy VK documentation-site URLs used
as LVK evidence, frontmatter repo://docs/... sources, and Claims evidence.resource
references. Update metadata-only dependencies through that lifecycle as well.
Do not hand-edit managed metadata, internal state, or audit history. Do not make
counts reach zero by merely deleting useful knowledge or its claims, renaming
links, or moving the same dependency into another citation format. Prefer
verified source/test/config references and supported Manifest evidence; preserve
useful historical context as labelled prose without requiring the retiring docs.
Relevant vendor documentation links remain allowed.

Required knowledge coverage

- Authentication and recovery: initial setup versus recovery, access.key and
  access.configured, configuration/DB consistency, closed failure states, the
  scope of a recovery session, key-loss limits and safe preservation before
  replacement. Keep LVK authentication separate from application/provider auth.
- Backup and restore: DB and journals, keys/settings/markers/certificates/secrets,
  home/repos/work volumes, consistency and ordering, archive verification,
  matching image/binary and DB state, and validation in a separate recovery
  deployment before switching traffic. Do not recommend blindly starting an
  older binary against a migrated DB.
- Runner uncertainty: reconnect the same identity/data root, preserve the journal,
  distinguish admission from execution and completion, verify containment
  settlement before operator resource recovery, and never infer that disconnect,
  timeout or cancellation proves termination. Do not invent a force-unlock path.
- Runtime/provider/configuration: independent file/execute permissions, allowed
  roots versus OS sandboxing, direct managed-Workspace boundaries, configuration
  keys/defaults, provider credentials and execution-account prerequisites, and
  limits on observation, cursor/output retention and recovery.
- Feature constraints and evidenced reasons, including pre-Manifest history:
  preserve supported distinctions and invariants; label historical status and
  unsupported reasons instead of converting old designs into current contracts.

Concise acceptance conditions

1. Readers can understand and perform the required operations from stable Wiki
   entry points without opening docs/. Compare high-risk recovery instructions
   against the relevant original docs and current source; retain prerequisites,
   settings, order, expected outcomes and failure limits. A missing critical step
   is an acceptance failure, not something to hide behind a generic "unknown".
2. No dependency on the retiring docs remains in public page links (including
   legacy VK docs URLs used as LVK evidence), frontmatter sources, or Claims
   evidence.resource. Report findings separately by layer with the exact search
   scope, completeness, stop reason and exclusions. A bare mention of the name
   docs/ in a policy explanation is not itself a dependency; do not use this
   distinction to hide an operational or evidentiary dependency.
3. Use the existing lifecycle and preserve its completion/source-drift checks.
   Report lifecycle completion and publication separately from migration/content
   acceptance. List affected pages handled, important operations checked,
   documents read selectively, unknown reasons and any unresolved dependencies or
   knowledge gaps. If acceptance is unmet, explicitly report the migration as
   incomplete with exact remaining items; do not claim success from finish,
   publication, a Current status, an empty plan, or one Sync receipt alone.

Ownership and execution boundaries

README is human-maintained with AI assistance and provides the overview and
links to stable Wiki operational entry points. The AI-maintained Wiki owns the
operational detail and may refer back to README for the human-maintained overview.
Do not move or rename README-linked page paths. Keep external-service UI steps
minimal; link to vendor documentation and document LVK keys/contracts locally.

This policy authorises the dedicated maintenance writer to update only the Wiki
through its normal lifecycle. It does not authorise deleting docs/, changing
source/tests/configuration/README or this user-authored file, issuing commits or
publishing independently of the LVK host, changing shared environments, or
executing destructive recovery procedures. Preserve this file during generation.
Respect existing read-only Review, host safety rules and source drift safeguards.


Migration acceptance follow-up: authentication recovery ordering

A content acceptance review of Wiki source 59bfe253 identified an incomplete
sequence in concepts/server-access.md, under the operator setup and recovery
instructions. Step 1 follows operations/server-container.md's consistent full-
volume backup procedure, which stops LVK; step 2 then uses docker compose exec
and the HTTPS recovery screen without first returning that same deployment to a
running state. Resolving documentation dependencies alone does not resolve this
acceptance finding.

During this reconciliation, verify and correct that sequence through the normal
OpenWiki page and Claims lifecycle. After the stopped backup has been verified,
explain how to restart the recovery target with the same image/binary, Compose
project, volumes and configuration, without an upgrade, reinitialization, or new
agent/Runner work. Confirm the server and HTTPS recovery entry are reachable
before issuing the recovery token against the same data directory and continuing
with the limited recovery session. Check the relevant source, existing deployment
commands and original authentication/backup procedures rather than inventing a
command. Preserve locked failure behavior, DB/key/marker consistency, and the
existing distinction between restoring the original key and configuring new
credentials. Do not suggest deleting the DB or access.configured.

Make the composed backup-to-recovery procedure self-contained and internally
consistent. Validate the steps and cross-links after writing them, and report
this finding separately from zero remaining docs dependencies. Preserve already
migrated knowledge and stable page paths; do not redo unrelated pages merely to
increase scope. Do not perform an actual shutdown, restore, key change, login or
other live recovery action as part of this documentation correction.


Migration acceptance follow-up: recorded rationale and provider configuration

The content review at cc339608 found two further gaps despite zero retiring-docs
dependencies. Reconcile them in this pass, using the existing stable page paths
and normal page/Claims lifecycle. Preserve the already repaired authentication
recovery sequence; do not repeat unrelated rewrites.

- In architecture/agent-runtime.md, preserve the documented historical reasons
  from docs/future/agent-runtime/architecture.md:713-725: why a single Message
  union was rejected (different semantics become optional/unknown fields), and
  why native input audit was needed to explain LVK requests, adapter translation
  and provider input for protocol changes, recovery and regression evidence.
  Corroborate the current contract/event/native_audit separation and actual input
  recording with runtime/mod.rs and local-deployment/process_host.rs. Label the
  recorded reasons as historical decisions rather than deriving intent solely
  from current code. Do not turn the old design into a current universal input
  write-ahead guarantee. Inspect the adjacent Tool Gateway and V1 knowledge-base
  exclusions for useful historical rationale, without reinstating them as current
  product non-goals or inventing subsequent decisions.
- In integrations/agent-providers.md, make the custom configuration location
  discoverable. Verify Codex's CODEX_HOME resolution, its empty/unset fallback to
  ~/.codex, and config.toml/auth.json and native MCP configuration against
  crates/executors/src/executors/codex.rs:69-79,428-445. Preserve the same execution
  account and configuration-directory requirement from docs/agents/claude-code.mdx:
  18-30, including CLAUDE_CONFIG_DIR when selected, and distinguish vendor CLI
  behavior from LVK's environment/profile auth probe in claude/auth.rs. Explain
  choosing the location before login/status and checking the selected profile's
  next execution in that same environment. Link to the standard Compose persistent
  home explanation. Do not copy credentials, display their contents, imply LVK
  login authenticates the provider, or repeat obsolete claims that the current
  container's Codex version is unpinned. Preserve uncertainty where vendor behavior
  is not established by the available evidence.

Validate that these reasons and configuration keys can be found in the Wiki
without opening the retiring docs. Keep current behavior separate from historical
rationale, and keep standard Compose login separate from custom locations. Do not
reintroduce retiring-docs links in pages, frontmatter or Claims; verify all three
layers again and preserve stable paths and this user-owned file. No live login,
account, key, environment, build or restore operations are needed or authorized
by this documentation correction.
