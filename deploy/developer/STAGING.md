# LVK contributor workflow

This optional kit is for developing LVK itself. Ordinary LVK users need neither
these commands nor an extra listener. No Docker socket or privileged container
is required. The standard server still runs the image's installed binary.

**Development → staging update and verification → release publication → stable update**

- Develop in a card worktree. Build/run an isolated instance on an internal port;
  run headless E2E against that port.
- Integrate into main, then build a pinned staging candidate. Expose it on HTTPS
  8443 for automated and human verification. No source watcher runs.
- Publish the verified commit through the existing GitHub image workflow. A
  successful image push does not update a running installation.
- Queue that immutable image digest for stable HTTPS 443 through `client.py`.
  End the requesting agent run so the host can observe an idle system.

Staging describes an environment, not a Cargo profile. This kit uses debug
binaries with built frontend assets. Release images use the existing optimized
CI build and image smoke tests. Both binaries honor `LVK_INSTANCE_DIR`.

## Isolated state

`LVK_INSTANCE_DIR` is an absolute directory containing `data/` (DB, settings,
attachments), `runtime/` (execution/worktree state), and `ports/` (discovery).
It is independent of debug/release. Without it, existing paths are unchanged.
Aliases and overlapping paths into the default stable data directory are
rejected. Never point a development process at stable data or copy a live
SQLite file without its WAL. Instance state is not a security sandbox against
code running under the same user.

The runner uses `/var/tmp/lvk-instances/NAME` (override its parent with
`LVK_INSTANCES_ROOT`). Every card must use a different NAME. Agent credentials
remain in the user's existing HOME; do not repurpose HOME for an instance.
Do not delete the retained staging source snapshot: debug frontend assets are
read from it. Completed builds have a `build.json` with commit/profile/identity.
A failed build never becomes a runnable candidate or replaces a running server.

## Card development

From the card's worktree, after installing dependencies:

```bash
pnpm run instance:build card-example
# Use the build_id printed by the command. Keep this process in the foreground:
pnpm run instance:run card-example BUILD_UUID 4200
```

The build includes uncommitted work and marks its metadata dirty. Run browser
checks with `LVK_E2E_BASE_URL=http://127.0.0.1:4200` in the same container. The
backend serves the built frontend directly; no Vite watcher or published Docker
port is needed. An optional REF after NAME builds a detached source snapshot.

## Staging

```bash
pnpm run staging:build main
pnpm run staging:run BUILD_UUID
```

The build resolves main to a commit, creates a retained detached worktree,
installs locked dependencies, builds the frontend and both Rust binaries, and
records the commit. It never changes main or a card's checkout. Only revisions
containing the instance-directory support can be built with this runner.

The server binds `127.0.0.1:4020`; optional nginx ingress exposes it on host 8443.
Staging has independent authentication state. On its first public start, run the
built `server setup-token` with the same `LVK_INSTANCE_DIR` and enter the key at
the staging URL. A stable setup key does not configure staging. When stable
and staging share a hostname/IP, use separate browser profiles: cookies are
host-scoped, so a staging login in the same profile replaces the stable cookie.
The instance lock prevents two staging servers sharing a DB. A separate build
lock permits building the next candidate while the current server remains up.
Stop the previous foreground instance with Ctrl-C **after its agent sessions
and workflows finish**, then start the candidate. Logs go to the foreground
console; the hosting terminal/job should retain them. A signal stops only this
instance's process group. Builds and runs refuse to start during host maintenance.

Starting a new schema can migrate staging data. Back up idle staging state before
such a switch; rollback requires the matching data snapshot, not just an old
binary. Stable data is never used. Health/E2E failure after starting a candidate
requires inspection; this kit does not silently roll its DB backward.

Shared staging must be operated through LVK resource coordination when registered.
Claim the canonical deployment, ports and data for the entire operation, and
include stop/cleanup and independent idle verification in the same operation.
Do not leave a detached server behind a completed test operation. Human preview
requires an explicitly managed long-lived service/terminal, not a forgotten test
process. Independent card tests use separate instance names and ports.

## One-time host migration from the old development container

This changes Docker publication and root-owned updater configuration. The
in-container update queue only changes image versions; it cannot perform this
migration. The host operator must do it once, during an idle maintenance window.

Before any image update, ensure free host disk space exceeds the backup size of
all persistent volumes plus 1 GiB. The updated host kit checks this before stopping
containers; older installations check only after stopping and must be checked
manually.

1. Publish/install an image from this revision using the existing release/update
   flow first. Keep the existing host updater configuration during this step.
2. Finish agents/workflows/queued follow-ups in both environments. Resolve pending
   image-update jobs before changing the host kit. Preserve old development
   volumes and checkout `dev_assets`; no automatic DB migration/import is done.
3. Back up the deployment Compose files and `/etc/lvk-updater.json`. Copy reviewed
   `compose.staging.yaml`, `compose.updater.yaml` and the new updater Python code
   to the host kit/deployment directory. Do not execute writable repository code
   as root without reviewing/copying it to the host-owned kit.
4. Stop the old `development` service using its existing Compose files. Do not run
   `down -v`. Remove `compose.development.yaml` from subsequent invocations and
   from the updater's `compose_files`; add `compose.staging.yaml`. Keep existing
   base, ACME and updater overlays. Set updater `services` to `["lvk"]`.
5. Recreate only `lvk` using that Compose set and the verified image. Preserve the
   existing Compose project name and volumes. The mappings are now host
   **443 → container 8443** and host **8443 → container 8444**.
6. Verify stable 443, build/start staging inside LVK, then verify authenticated
   staging 8443, its API, and browser E2E. With no staging process, its ingress
   returns an upstream-unavailable response; stable health is independent.

For new installations, install.py defaults to stable-only updates. Pass
`--legacy-development` only when retaining the old two-container arrangement;
that adds `compose.updater-legacy-development.yaml` as a compatibility overlay.
The base Compose file and ordinary image startup do not enable staging.

The host updater waits for managed coding agents, not instance/build processes.
Container replacement stops staging and other non-agent processes. Start staging
explicitly after an image update; it is not automatically restarted with stale
artifacts. Host kit updates remain separate from image updates.
