# Markdown reading surfaces

Run from the repository root:

```sh
node --test tests/markdown/roundtrip.test.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/markdown/reading.mjs
```

The roundtrip tests use the production quote adapter with Lexical's actual
Markdown converters. The browser fixture mounts the real WYSIWYGEditor,
MarkdownPreview and KanbanIssuePanel with inert callbacks and local attachment
metadata. It never starts a backend, reads a database or executes an agent.

Coverage includes matching body typography, light/dark and narrow/wide panes,
long code/tables, explicit decision/standard alerts versus ordinary quotes,
Japanese/duplicate headings, document switches, pane-only outline navigation,
keyboard focus, no onChange from viewing/navigation, edit/export/reload,
source copy, safe links, attachment and inline-code file actions, and existing
sanitized details behavior. It does not prove authenticated application/API
integration, external preview routing, OS clipboard interoperability or physical
mobile behavior.

Screenshots and measurements are written to ignored `.build/markdown-reading-review/`.
For a before comparison, set `MARKDOWN_BASELINE` to a pinned commit; the script
loads product source and CSS from Git without changing the checkout. For example:

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers MARKDOWN_BASELINE=12f505d7da23efd696312aae8c088fa4dadae01b node tests/markdown/reading.mjs
```

The break sample measures text line positions: MarkdownPreview collapses ordinary
soft breaks, both paths preserve explicit hard breaks, and Lexical retains its
existing line rendering. It also checks all six alert types across multiple
paragraphs, re-render, edit/export/reload and exact source copy, including that
hiding a marker span does not leave an extra first line. The adjacent DECISION /
IMPORTANT sample produces an investigation-only teal-border candidate image;
that candidate is never written to product CSS.

Visual context: this inert fixture uses `bg-primary` for the comparison surface.
It does not reproduce the authenticated application's `bg-secondary` Issue pane;
these screenshots cannot establish that parent surface's contrast. Product and
fixture background colors are not changed by the break fix.

These are regression fixtures, not a public preview server. Use the existing
managed DevServer workflow for preview2; verify its authentication, isolated
backend data context and ownership before startup. Do not expose this fixture
through a new unauthenticated route or connect it to shared development data.
