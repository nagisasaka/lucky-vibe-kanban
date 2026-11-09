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

Screenshots and measurements are written to ignored `.build/markdown-reading/`.
For a before comparison, set `MARKDOWN_BASELINE` to a pinned commit; the script
loads product source and CSS from Git without changing the checkout. For example:

```sh
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers MARKDOWN_BASELINE=6ad758d6ee1d74785a6e70e0f17ef93a0ab43d8e node tests/markdown/reading.mjs
```

These are regression fixtures, not a public preview server. Use the existing
managed DevServer workflow for preview2; verify its authentication, isolated
backend data context and ownership before startup. Do not expose this fixture
through a new unauthenticated route or connect it to shared development data.
