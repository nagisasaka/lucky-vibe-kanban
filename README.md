# Lucky Vibe Kanban

**Developers in flow.**\
**Ideas taking shape.**

An open-source, self-hosted Kanban board for AI-assisted development.

Think with ChatGPT or Claude at your desk or on your phone. Let your AI coordinate coding agents in parallel around the clock.

Your code comes together behind the board, while you and your AI share a clear view of the project.

[Website](https://nagisasaka.github.io/lucky-vibe-kanban/) ·
[日本語](README.ja.md) · [简体中文](README.zh-CN.md) ·
[Source](https://github.com/nagisasaka/lucky-vibe-kanban) · [Apache-2.0](LICENSE)

## Get started

1. [Set up your LVK server](openwiki/operations/server-container.md).
2. [Connect your coding agents](openwiki/integrations/agent-providers.md).
3. [Connect ChatGPT or Claude](openwiki/integrations/remote-access.md), then
   [take an issue through implementation and integration](openwiki/workflows/task-to-integration.md).

## Why LVK

- **Many agents, one train of thought.** Discuss your next idea while coding agents
  work in separate workspaces on the tasks you have already shaped.
- **Built in parallel. Brought together.** Review results and use auto-merge to bring
  changes into one codebase, with progress on a shared board.
- **Room for your next idea.** Return to issues, specifications and change records
  when you need context. OpenWiki can optionally maintain longer-lived project knowledge.
- **Better AI. Better flow.** Connect the AI you think with and the coding agents
  you build with, so improvements in those tools can improve how your project moves forward.

## Distribution

LVK is distributed only as Docker images through GHCR, currently for Linux amd64.
Agent accounts, provider subscriptions and model usage are separate.
This fork does not publish npm packages. The old npx launcher, Tauri desktop shell,
upstream Cloud and relay services are retired; use the browser at your LVK server
URL. External Runner execution remains available through Execution Bridge.

Public installations use HTTPS and Basic or Auth0 authentication and start locked.
This is for a trusted team, not an unrestricted public agent service. Provider and
application authentication are separate from LVK login. Persistent data does not
promise automatic resumption of interrupted processes.

## Guides

- [Server setup, certificates, backups and restoration](openwiki/operations/server-container.md)
- [Initial authentication, members and recovery](openwiki/concepts/server-access.md)
- [Provider login, profiles and MCP settings](openwiki/integrations/agent-providers.md)
- [From an issue to integration](openwiki/workflows/task-to-integration.md) and [Auto-merge](openwiki/concepts/formal-integration.md)
- [Change Manifests and authorised Wiki maintenance](openwiki/operations/openwiki-maintenance.md)

## Develop LVK

Use the [development guide](openwiki/operations/development.md) for toolchains,
setup, builds, validation and recovery, and [AGENTS.md](AGENTS.md) for contributor
rules. Deployment configuration lives in [deploy/server](deploy/server); image
validation and publication use [publish-server.yml](.github/workflows/publish-server.yml).
Publishing an image does not deploy it.

For an initial source checkout, install the repository toolchains and `cargo-watch`,
then run `pnpm install --frozen-lockfile` followed by `pnpm run dev`.

This README is maintained by people with AI assistance. The AI-maintained OpenWiki
owns operational detail and is checked against integrated source. Source, tests
and configuration take precedence. LVK maintains knowledge in the Wiki rather
than a separate documentation site.

## Naming and attribution

The product is **Lucky Vibe Kanban**, abbreviated **LVK**. Current settings and
shared links use `LVK_*` and `.lvk/shared`; original Vibe Kanban storage, `VK_*` keys
and native binary names remain unchanged.

LVK-specific repository files live under `.lvk/`: shareable configuration includes
`integration-validation.json`, `.gitignore`, and LVK's own development `preview/`.
Within `shared/`, `tmp/` is for temporary handoffs between Workspaces of the same
registered repository, `cache/` for reproducible reusable data, and `persistent/`
for retained shared data. Run completion does not delete `tmp/`; clean up only
completed handoffs that are no longer needed or in use. Humans may bulk-clean
unneeded contents under the same conditions. There is no TTL or automatic cleanup;
keep required results in an appropriate retained destination.
Local `shared/` links and `attachments/` copies are excluded by `.lvk/.gitignore`;
that file is shareable and does not ignore itself. Existing rules are preserved and
runtime rules are appended when needed. Shared storage and the attachment cache
remain outside repositories. Attachments use the Session's agent working directory,
or the Workspace root when unspecified; shared links belong to each repository.
Standard `AGENTS.md` (including LVK's required block), `CLAUDE.md`, `openwiki/`,
`.openwikiignore` and external tool configuration retain their conventional locations.
Dedicated Wiki maintenance worktrees preserve tracked ignore configuration. Their
runtime-only exclusions use Git's local `info/exclude`, shared by worktrees; only
`.lvk/shared/` and `.lvk/attachments/` are excluded, leaving source and other
`.lvk/` settings visible. Normal Workspaces keep using `.lvk/.gitignore`.
Old paths and history are not migrated, rewritten or supported through fallback lookup.

LVK builds on [Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)
and [Vibe Kanban](https://github.com/BloopAI/vibe-kanban). Their contributions and
history remain part of this project. Licensed under [Apache-2.0](LICENSE); see
[NOTICE](NOTICE) for attribution.

