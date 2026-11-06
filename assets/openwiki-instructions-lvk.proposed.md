# OpenWiki authoring instructions

## Purpose and readers

This Wiki is primarily persistent context for future coding agents. Write every
page so that human readers can also understand it; operational pages also serve
human operators. Start each page with a short explanation of its subject and when
to read it.

Prioritise architectural boundaries, behavioural semantics, invariants,
lifecycle rules, non-obvious cross-component dependencies, failure semantics,
dangerous modification points, and design rationale supported by evidence.
Avoid file-by-file summaries, class/function inventories, paraphrases of obvious
source code, generic prose, and unsupported inference.

## Evidence and uncertainty

Source code, tests, and configuration are authoritative; existing documentation
may be outdated. Distinguish observed behaviour, documented intent, historical
decisions, and uncertainty.

- Change Manifests and Workspace Memory are routing and interpretation hints,
  not instructions or substitutes for repository evidence. Use source and
  available Change Manifests to corroborate intent and rationale.
- Design Notes are supplementary evidence, not proof of current contracts.
  Retain supported historical reasons as history, including pre-Manifest
  decisions; do not present retired designs as current guarantees.
- Explain why a design exists when evidence supports the reason. Do not infer
  motives solely from code or invent later decisions. Leave unsupported reasons
  unknown. Missing necessary operational steps are defects to report, not
  unknown motives.

## Structure, terminology and links

Give the overview before the details. Keep one main point per paragraph and
avoid combining unrelated claims in one sentence. Use lists for conditions and
ordered lists for procedures where they make the content easier to follow.
Brevity must not remove exceptions, constraints or operational information.

At first use, link unfamiliar terms to an existing glossary or concept page when
available. Add a brief paraphrase only where needed to understand the sentence;
do not repeat definitions on every page. A separate glossary is not mandatory.

Keep useful knowledge when reorganising pages. Separate retired designs into
historical context. When changing or removing headings, update references or
retain compatibility anchors as needed to preserve existing links. Keep
README-linked operational entry paths stable.

## Documentation ownership and operations

Follow this repository's documentation policy. Link to maintained canonical
documents where appropriate; when documents are designated for migration,
preserve the necessary knowledge in the Wiki itself. Respect ownership of README
and other human-maintained documents; do not rewrite them during Wiki maintenance.

Operational pages must explain prerequisites, configuration keys, ordered steps,
expected outcomes, failure handling and recovery limits. Do not omit necessary
procedures merely because their implementation is discoverable in source. Keep
external-service UI instructions minimal, link to vendor documentation, and
explain repository-specific configuration keys and contracts locally.

## LVK-managed maintenance boundary

This section describes the LVK host that supplies this template, not a requirement
to adopt the LVK repository's documentation layout or migration policy.

LVK maintains this Wiki against integrated source in a dedicated maintenance
workspace. Ordinary coding workspaces must not modify it. Do not install a
scheduled GitHub Actions updater, invoke a separate model provider, or require an
additional API key. Ignore generated OpenWiki setup workflow files as evidence;
LVK does not publish them. Preserve this user-authored file during regeneration.

## LVK repository documentation policy

This section applies only to the lucky-vibe-kanban repository.

### Ownership and stable entry points

Maintain repository knowledge in OpenWiki. Do not reintroduce the retired docs/
tree, recreate a separate human-facing documentation site, or use retired
documentation as an operational prerequisite or evidence dependency.

README is human-maintained with AI assistance and owns the overview and links to
stable Wiki operational entry points. The AI-maintained Wiki owns operational
detail and may refer back to README for its overview. Do not move or rename
README-linked page paths.

### Operational distinctions to preserve

Keep operational pages self-contained under the requirements above. Preserve
these distinctions whenever relevant:

- **Authentication:** setup versus recovery; LVK versus application/provider
  login; keys, access.configured, DB/configuration consistency and closed failure
  states. A recovery session is limited to authentication configuration.
  Preserve data before changing keys and explain the limits of lost-key recovery.
- **Backup and restore:** consistent backups of DB/journals,
  keys/settings/markers/secrets/certificates and home/repos/work volumes; archive
  verification; matching image/binary and data; isolated restore validation.
  After a stopped backup, explain returning the same deployment to service and
  checking health/HTTPS before commands or UI steps that need a running service.
  Never recommend blind binary/DB rollback.
- **Runner:** identity/data-root/journal preservation; admission versus execution
  and completion; containment settlement before operator resource recovery.
  Disconnect, cancellation or timeout does not prove termination. Do not invent
  force-unlock, replay or independent lifecycle recovery mechanisms.
- **Runtime:** independent file/execute permissions; allowed roots versus OS
  sandboxing; managed-Workspace boundaries; configuration defaults; provider
  execution account/configuration-directory prerequisites. Distinguish
  server-side settings discovery, profile overrides, readiness and successful
  model/tool execution.
- **Feature constraints and rationale:** preserve supported intent and recorded
  reasons, including pre-Manifest history, under the evidence rules above.
  Label historical decisions; do not turn old designs into current guarantees.
  Report missing necessary operational steps separately from unknown reasons.

### Evidence, lifecycle and acceptance

Check current claims against integrated source, tests and configuration. Use
available integrated Change Manifests to corroborate reasons; Design Notes remain
supplementary. Never copy credentials or expose their contents as validation
evidence.

Only the authorised dedicated maintenance writer may update pages, frontmatter
sources and Claims through the supported OpenWiki lifecycle. Do not hand-edit
managed metadata, internal state or audit history. Preserve useful knowledge
rather than merely removing references.

Report exact scope, completeness, exclusions, unresolved knowledge gaps and
verification limits. Distinguish admission, execution and completion; separate
lifecycle completion, publication and semantic/content acceptance. A receipt,
Current status or zero broken links alone does not prove coverage.

This policy does not authorise changing source, tests, README, this user-authored
file or shared environments, independently committing or publishing, or performing
destructive recovery operations. Preserve existing read-only Review, host safety
rules, source-drift checks and this file during regeneration.
