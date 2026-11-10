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
The Issue component below it now has a `bg-secondary` outer surface matching
the Issue pane token; it still uses synthetic data and inert callbacks, not an
authenticated application session. Product background colors remain unchanged.

These are regression fixtures, not a public preview server. Use the existing
managed DevServer workflow for preview2; verify its authentication, isolated
backend data context and ownership before startup. Do not expose this fixture
through a new unauthenticated route or connect it to shared development data.

For Issue title / body heading comparisons and wrapping measurements:

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers MARKDOWN_HEADINGS=1 node tests/markdown/reading.mjs
```

This also runs the existing reading regressions. The additional comparison uses
the actual Issue panel with a long Japanese title at 390/800px in both themes.
It captures the original scale, two candidate scales and the final product CSS,
checks title text/scroll dimensions, heading order and title/body editing, and
compares document heading sizes between both renderers in view/edit states.
Images and measurements go to ignored `.build/markdown-heading-review/`. Candidate
CSS is injected only into the test browser; no saved Markdown or backend is used.
