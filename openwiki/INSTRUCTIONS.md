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

Permanent LVK documentation policy

This policy applies only to this LVK repository. Maintain repository knowledge
in OpenWiki. Do not reintroduce the retired docs/ tree for LVK documentation.
Do not recreate a separate human-facing documentation site or use
retired documentation as an operational prerequisite or evidence dependency.
README is human-maintained with AI assistance and contains the overview and
links to stable Wiki operational entry points. The AI-maintained Wiki owns
operational detail and may refer back to README for its human-maintained overview.
Do not move or rename README-linked page paths.

Keep operational pages self-contained: retain prerequisites, configuration keys,
ordered steps, expected results, failure handling and recovery limits. In
particular, preserve the following distinctions when relevant:

- Authentication setup versus recovery; LVK versus application/provider login;
  keys, access.configured, DB/configuration consistency and closed failure states.
  A recovery session is limited to authentication configuration. Preserve data
  before changing keys and explain the limits of lost-key recovery.
- Consistent backups of DB/journals, keys/settings/markers/secrets/certificates and
  home/repos/work volumes, archive verification, matching image/binary and data,
  and isolated restore validation. After a stopped backup, explain returning the
  same deployment to service and checking health/HTTPS before commands or UI steps
  that need a running service. Never recommend blind binary/DB rollback.
- Runner identity/data-root/journal preservation, admission versus execution and
  completion, and containment settlement before operator resource recovery.
  Disconnect, cancellation or timeout does not prove termination. Do not invent
  force-unlock, replay or independent lifecycle recovery mechanisms.
- Independent Runtime file/execute permissions, allowed roots versus OS sandboxing,
  managed-Workspace boundaries, configuration defaults and provider execution
  account/configuration-directory prerequisites. Distinguish server-side settings
  discovery, profile overrides, readiness and successful model/tool execution.
- Feature constraints and supported intent/rationale, including pre-Manifest
  history. Preserve recorded reasons as labelled historical decisions where
  appropriate; do not derive motives solely from code, invent later decisions,
  or turn old designs into current guarantees. Missing reasons remain unknown;
  missing necessary operational steps are defects to report, not unknown motives.

Check current claims against integrated source, tests and configuration and use
available integrated Change Manifests to corroborate reasons. Design Notes remain
supplementary. Keep external-service UI steps minimal, link to vendor documents,
and explain LVK configuration keys and contracts locally. Never copy credentials
or expose their contents as validation evidence.

Update pages, frontmatter sources and Claims through the supported OpenWiki
lifecycle. Preserve useful knowledge rather than merely removing references.
Do not hand-edit managed metadata, internal state or audit history. Report exact
scope, completeness, exclusions, unresolved knowledge gaps and verification
limits. Separate lifecycle completion, publication and semantic/content acceptance;
a receipt, Current status or zero broken links alone does not prove coverage.

Only the authorised dedicated maintenance writer may update the Wiki through
that lifecycle. This policy does not authorise changing source, tests, README,
this user-authored file or shared environments, independently committing or
publishing, or performing destructive recovery operations. Preserve existing
read-only Review, host safety rules, source-drift checks and this file during
regeneration.
