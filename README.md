# Lucky Vibe Kanban (LVK)

**Plan work on a kanban board, run coding agents, and review their changes.**

LVK is a self-hosted development workspace with Git worktrees, multiple sessions,
visual agent workflows, local integration and OpenWiki. Run it
locally or on your own server so agent work can continue while your laptop is
disconnected. Agent accounts, provider subscriptions and model usage are separate.

[日本語](README.ja.md) · [简体中文](README.zh-CN.md) ·
[Source](https://github.com/nagisasaka/lucky-vibe-kanban) · [Apache-2.0](LICENSE)

## Distribution

LVK is distributed only as Docker images through GHCR, currently for Linux amd64.
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
shared links use `LVK_*` and `.lvk-shared`; original Vibe Kanban storage, `VK_*` keys
and native binary names remain unchanged.

LVK builds on [Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)
and [Vibe Kanban](https://github.com/BloopAI/vibe-kanban). Their contributions and
history remain part of this project. Licensed under [Apache-2.0](LICENSE); see
[NOTICE](NOTICE) for attribution.
