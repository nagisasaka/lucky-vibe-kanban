# Lucky Vibe Kanban (LVK)

**Plan work on a kanban board, run coding agents, and review their changes.**

Lucky Vibe Kanban is a self-hosted development workspace built on the Vibe Kanban
and Lucky Vibe Kanban projects. Run it on your computer, or on your own server so
agent sessions can continue while your laptop is disconnected.

[日本語](README.ja.md) · [Server setup](docs/self-hosting/server-container.mdx) ·
[Source](https://github.com/nagisasaka/lucky-vibe-kanban) · [Apache-2.0](LICENSE)

## What you can do

- Organise issues and run coding agents such as Codex and Claude Code in Git worktrees.
- Keep multiple sessions and workspaces, inspect execution history, and review diffs.
- Compose agent steps into visual workflows, including parallel branches and conditions.
- Integrate completed local-board work through a dedicated validation workspace.
- Maintain repository memory with OpenWiki, with reconciliation after source integration.
- Run the application and agents on an always-on server through HTTPS and Basic or Auth0 authentication.

Agent accounts and provider subscriptions are configured separately. LVK does not
provide included model usage or a public, unrestricted agent service.

## Distribution

LVK is distributed **only as Docker images** through GHCR, initially for Linux
amd64. This fork does not publish an npm package. For local development, run from
source using the instructions below. The legacy npx launcher has been removed;
see the [migration guide](docs/self-hosting/server-container.mdx#legacy-npx-cli-migration)
for server, MCP and desktop entrypoints.

## Run on your own server

Use the [single-user server guide](docs/self-hosting/server-container.mdx) and the
files in [`deploy/server`](deploy/server). The published Linux amd64 image contains
LVK, nginx, Codex CLI, Git, Node.js, pnpm and a Rust development toolchain.

- A stable public IP is sufficient: **no purchased domain is required**.
- Let's Encrypt certificates are obtained and renewed automatically.
- HTTPS and Basic or Auth0 authentication protect the application. Public installations start locked; run `docker compose exec lvk server setup-token`, then complete the [HTTPS setup screen](docs/self-hosting/authentication.mdx).
- Docker volumes retain repositories, application data and agent credentials.
- A server or container restart stops running processes; persistence does not
  promise automatic resumption of interrupted agent work.

This is for one trusted user. Wildcard project previews require additional setup.
The upstream Cloud and relay Remote Access services are retired. Access LVK at
your server URL. External Runner execution remains available through Execution Bridge.

## Develop LVK

Use the Node.js and pnpm versions in `package.json` and the Rust toolchain in
`rust-toolchain.toml`. Install `cargo-watch`, then:

```bash
pnpm install --frozen-lockfile
pnpm run dev
```

Vite reloads frontend changes. `cargo watch` rebuilds and restarts the Rust
backend. Keep the Cargo target directory between runs to reuse build output.

For development on a server, follow the
[source-development setup](docs/self-hosting/server-container.mdx#develop-lvk-on-the-server).
The image includes headless Chromium for browser E2E. LVK contributors can also
install the optional [host update service](deploy/developer/README.md) to publish
and adopt their fork's images from inside their own LVK environment.
It runs a separate development container using an existing published image:
source edits do **not** require an image rebuild. The development app has its own
HTTPS port, database, home and work volumes. The stable app continues to manage
agent work while the development app restarts.

| Command                      | Purpose                                                              |
| ---------------------------- | -------------------------------------------------------------------- |
| `pnpm run dev`               | Local frontend and backend development                               |
| `pnpm run dev:container`     | Fixed development ports for container access                         |
| `pnpm run local-web:preview` | Build and serve a mobile preview on port 4030 (existing API on 4021) |
| `pnpm run check`             | Web TypeScript checks and server Rust packages                       |
| `pnpm run lint`              | Web lint, server Rust Clippy and unused translation keys             |
| `pnpm run format`            | Rust and web formatting                                              |
| `pnpm run server:check`      | Server configuration and ingress tests                               |
| `pnpm run backend:test`      | Server Rust workspace tests                                          |
| `pnpm run generate-types`    | Generate shared TypeScript types from Rust                           |

The Tauri desktop shell is retired. Use the browser with the Docker server or
the source development environment; browser notifications and editor integrations
remain available.

Shared type generation runs in the public server workspace, including the local
board contracts. Do not edit generated files in `shared/` directly.

Release builds and publication run in GitHub Actions through
[`publish-server.yml`](.github/workflows/publish-server.yml). The workflow tests
the server image before publishing a versioned image to GHCR. Routine source
checks and hot reload do not require a release build.

### Mobile development preview

With the development backend already running, run `pnpm run local-web:preview`
and open `http://<server-address>:4030` on your phone. Publish port 4030 if using
Docker. This serves bundled assets instead of Vite's development modules and
proxies HTTP and WebSocket API requests to the existing backend on port 4021.
Set `FRONTEND_PORT` and `BACKEND_PORT` to override these ports. The proxy preserves
the browser Host header for same-origin checks.

To use the existing container frontend port instead, start the development stack
with `FRONTEND_MODE=preview pnpm run dev:container` (default frontend port 4020).
Use this in place of the normal stack start command, not alongside it. Omit
`FRONTEND_MODE` to return to hot reload. For the managed Docker development
profile, set `LVK_DEV_FRONTEND_MODE=preview` in the Compose environment and
recreate the development service; its existing HTTPS URL stays the same.

Restart the command after source changes to rebuild; this mode has no hot reload.
Its `dist-preview` output is separate from Cargo's embedded frontend assets.
Builds default to a 4 GiB Node heap, matching the Docker build; an existing
`NODE_OPTIONS` value overrides this default. This is a development preview;
use the server image for production deployment.

## Workspaces and repository memory

Each registered repository can share local files through `.lvk-shared/persistent`
and `.lvk-shared/cache`. These are Git-ignored links shared across its workspaces;
configure language-specific caches explicitly and back up important local files.

[OpenWiki](docs/workspaces/openwiki.mdx) is configured per repository. Coding
agents contribute change manifests; repository memory is reconciled after source
integration. See the [integration operation guide](docs/design/parallel-integration-implementation.md#operation-and-recovery)
for source reservations, validation, publication and recovery.

## Naming

The product is **Lucky Vibe Kanban**, abbreviated **LVK**, and the repository
is `lucky-vibe-kanban`. Current settings and shared links use `LVK_*` and
`.lvk-shared`. Original Vibe Kanban storage, `VK_*` keys and native binary names
are unchanged.

## Acknowledgements and licence

LVK builds on [Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)
and [Vibe Kanban](https://github.com/BloopAI/vibe-kanban). Their contributions and
history remain part of this project. Licensed under [Apache-2.0](LICENSE); see
[NOTICE](NOTICE) for attribution.
