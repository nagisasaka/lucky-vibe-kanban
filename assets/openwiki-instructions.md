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

- Change Manifests are routing and interpretation hints,
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
