> For LVK self-development, use the [development → staging → release → stable workflow](STAGING.md).
> The default updater now manages only `lvk`. The separate `development` service
> described below is a legacy compatibility option (`install.py --legacy-development`).
> Ordinary LVK users do not need either contributor setup.

# Develop LVK using your own LVK server

This optional Linux/systemd host kit updates a developer's LVK installation
from their own GitHub repository. It is **not included in the application
image**. Normal LVK users do not need this service. The standard server image
does include headless Chromium, Playwright and its system dependencies.

## Development loop

1. Fork the repository and enable Actions. Start the official stable image with
   the server Compose kit and its optional source development service.
2. Check out your fork under `/repos`, configure Git push authentication for your
   own fork, and log in to your agent. Keep stable and development homes separate.
3. Edit source from stable LVK, then run `pnpm run server:e2e` against the
   development URL. Source changes do not require an image build.
4. Integrate the changes you want to adopt, push a new `v*` version tag and wait
   for `publish-server.yml` to succeed. Manual workflow dispatch validates but
   does not publish. Copy the published **digest**, full source SHA and run ID.
5. Submit an update, then end the requesting agent run. Check the saved job
   status from a new run after the update. A pending request does not force any
   active agent, goal, terminal, resource operation or workflow to stop.

Git push authentication is separate from reading public release metadata. Use
credentials restricted to your fork, such as a write-enabled
[repository deploy key](https://docs.github.com/en/authentication/connecting-to-github-with-ssh/managing-deploy-keys).
Keep its private key in the stable service's persistent home, never in Git or
the application image. Configure the checkout's SSH push URL and key before
attempting a release. The public release client itself needs no GitHub API token.

The development service watches **one** checkout (`LVK_DEV_REPO`). It does not
automatically follow the card that requested testing. Select the intended
worktree when starting a preview, or use separate ports. Serialise use of a
shared preview with resource coordination. The updater does not switch branches,
pull source, commit files, merge cards, or automatically deploy every push.

## Install on the Docker host

Requirements: Linux Docker Engine/Compose, systemd, Python **3.11+**, Bash,
OpenSSL, GNU tar, and enough free storage for a full persistent-data backup.
The current kit supports the repository's `lvk` and `development` services,
local Docker named volumes and its standard source runner. Custom runners or
unknown managed-agent states fail closed until their owner reviews the setup.
Bootstrap a release containing this kit once through SSH before queueing
updates: older releases cannot fence agent admission and drain in-flight HTTP
handlers. Managed development checkouts must support the same maintenance barrier.

Copy this directory from the reviewed source revision to the host. The host
installation is independent of the writable checkout: application agents
cannot replace root-executed updater code. Update this kit separately via SSH
when its host code changes.

The default stable-only updater does not require Basic credentials. It verifies
local backend readiness with the container's existing owner-only internal
capability, which is not exposed through nginx. This is a trusted local smoke
check, not proof of real Auth0 Web login or ChatGPT interoperability.

Only the legacy development overlay requires `secrets/e2e.json` with the existing
HTTPS Basic username/password (JSON keys `username` and `password`). Keep it out
of Git and chats. The installer restricts it to root and group 10001.

Run from this directory on the host, substituting your deployment and fork:

```bash
sudo python3 install.py \
  --deployment /opt/lvk-server \
  --repository YOUR_ACCOUNT/lucky-vibe-kanban \
  --compose-file compose.yaml \
  --compose-file compose.acme.yaml \
  --compose-file compose.development.yaml \
  --legacy-development
```

Include `-f compose.updater.yaml` in subsequent Compose operations. Recreate
the application services after existing work finishes to mount the request,
read-only status, maintenance and browser-credential paths. The timer is
`lvk-updater.timer`; configuration is `/etc/lvk-updater.json`; the reviewed host
scripts are in `/opt/lvk-developer`. Preserve the existing Compose project name
and volumes. Do not run `down -v`.

The default state directory is `/var/lib/lvk-updater`. If you choose another,
also set `LVK_UPDATER_STATE` in the deployment `.env`. Public GHCR images and
public Actions metadata need no GitHub token. For private forks/rate limits,
configure host Docker's pull credentials and an optional `github_token_file`
containing a token with **Actions: read** only for the configured repository.
Do not mount the host Docker socket or this token into application containers.

## Browser E2E

From the checkout inside stable LVK (the overlay supplies the URL and credential
file):

```bash
pnpm run server:e2e
pnpm run workflow:e2e
```

`server:e2e` opens the real app, checks rendering, reload and backend responses,
and verifies anonymous HTTPS requests are rejected. It is read-only: no paid
agent run is started and no production cards are deleted. Extend these tests
for feature-specific flows using a dedicated development database.
`workflow:e2e` runs the separate UI fixtures at port 4175. Both default to bundled
Chromium; an explicit `PLAYWRIGHT_CHANNEL` selects a branded browser for the UI
fixtures if you have installed one.

For a different deployment, explicitly set `LVK_E2E_BASE_URL` and
`LVK_E2E_CREDENTIALS_FILE`. TLS validation stays enabled. Reports are under
`playwright-report/server` and `test-results/server`. These can contain private
application content. Traces are off unless `LVK_E2E_TRACE=1`.

The image installs Playwright's pinned browser revision outside mounted homes
at `/opt/lvk-browsers`. Use the repository's lockfile. Other projects with a
different Playwright revision can download their own browser without root:

```bash
export PLAYWRIGHT_BROWSERS_PATH="$HOME/.cache/ms-playwright"
pnpm exec playwright install chromium
```

The generic image's browser/system-library smoke test is also available as
`node /opt/lvk-browser/smoke.mjs` with `LVK_E2E_BASE_URL` set.

## Queue an update

For a public fork, the client resolves the source SHA, successful CI run and
GHCR digest without needing a GitHub token in the agent container:

```bash
python3 deploy/developer/client.py release \
  --repository YOUR_ACCOUNT/lucky-vibe-kanban \
  --tag v0.1.44-lvk.1-server.8
```

From the checkout inside stable LVK, substitute the three values from the
successful version-tag release:

```bash
python3 deploy/developer/client.py submit \
  --image ghcr.io/YOUR_ACCOUNT/lucky-vibe-kanban@sha256:IMAGE_DIGEST \
  --revision FULL_COMMIT_SHA \
  --run-id ACTIONS_RUN_ID
python3 deploy/developer/client.py status JOB_UUID
python3 deploy/developer/client.py cancel JOB_UUID
```

The client saves a request and returns immediately. **End that agent run**;
waiting synchronously from a running agent would prevent its own update.
Cancellation only affects queued/waiting jobs. Successful status includes the
exact request so the next agent can verify which image was installed. Running
processes are never restored magically: any further work is a new execution
using the saved session/history.

The host checks the allowed GHCR repository, digest, image source/SHA/run-ID
labels and successful version-release workflow. Publishing credentials for that
repository are a trust boundary; these checks are not a signed supply-chain
attestation. It tests the image against disposable volumes before interrupting
the installation. That test does not use the real agent's credentials or data.

## Update and recovery contract

The service waits only for LVK-managed coding agents to finish in each managed
application. Agent runs, provider attempts and their process registrations must
be terminal; unknown agent states and unreadable databases still block updates.
Shells, builds, browsers, development servers, workflow/Integration parent
records, active Goal metadata, schedules and shared-resource records do not
block an update by themselves. These non-agent processes will be stopped by
container replacement; in-memory queued follow-ups are not preserved.

After closing the maintenance gate, the host drains in-flight HTTP handlers.
New mutating requests and WebSocket upgrades receive 503 while read-only
internal health/browser probes remain available. It freezes the containers and
checks managed-agent records again to close the admission race. If an agent
appeared, it unfreezes them and waits. Otherwise it terminates the frozen
containers without resuming admission. SQLite WAL recovery preserves committed
transactions. Image identity, backup, health and rollback checks still apply.
The debug database is in the checkout's `dev_assets`; the release database is
in its persistent home.

With both containers stopped, the service archives all their persistent named
volumes together with deployment configuration. Only the reproducible
`development-work/lvk-target` build cache is excluded. Backups contain secrets;
only root can read them. Backups are kept, never automatically pruned. Monitor
disk usage and remove old, reviewed backups on the host as needed.

The exact image digest is applied, both services restart, and browser/backend
health checks pass before the optional maintenance gate reopens HTTPS. If
verification fails, the service verifies archive checksums and restores the
previous **image and corresponding persistent data**. Old images are never
pointed at a migrated database as a substitute for restoration.

If the host/updater is interrupted mid-transaction or rollback cannot safely
finish, the persisted job becomes `recovery_required` and blocks later updates.
The service does not guess which snapshot to destroy. Inspect it via SSH:

```bash
sudo journalctl -u lvk-updater.service -n 100
sudo cat /var/lib/lvk-updater/jobs/JOB_UUID.json
sudo python3 /opt/lvk-developer/updater.py --recover JOB_UUID
```

Recovery before a completed backup resumes the original containers without
restoring data. Recovery after application of an image restores the recorded
backup, so inspect the job before invoking it. Keep SSH as the independent
repair route. Updating the source checkout, the host kit itself, Docker, or the
host OS remains a separate operation.
