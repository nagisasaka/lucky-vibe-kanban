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
