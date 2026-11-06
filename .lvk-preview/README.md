# LOCAL-300 preview2

The existing managed DevServer runs `exec bash .lvk-preview/start.sh` in this
checkout. The orchestrator owns its stop/start; do not launch a second long-lived
supervisor from a coding run.

`run.py` reads `.build/design-preview/backend/manifest.json`, resolves its
`binary` relative to this checkout and verifies `sha256` before launch. A missing
or changed candidate fails closed. The prepared manifest also records the source
revision and build metadata; the manifest and binaries are local artifacts.

A managed restart is required to use a newly prepared backend. HMR updates only
the frontend. Keep the existing instance at `.build/design-preview/instance`,
including its authentication, database, attachments and `runtime/worktrees`.
The public origin remains `https://preview2.luckyvibekanban.com`, with Vite on
127.0.0.1:18444 and the dedicated backend on 127.0.0.1:18446. This script does not
modify dev/prod launch configuration or use their data.

The previous running release is preserved at
`/var/tmp/lvk-instances/dev/releases/d21d587f-8e61-4ae1-bde8-d745c34bec36/server`.
Do not replace that artifact. Candidate preparation does not mean cutover or
successful validation against the public preview's new backend.

## Preview-only integration boundary

This LOCAL-300 branch is an internal preview continuation, not a product merge
candidate. The 2026-11-05 sync includes main
`7d4fdc8cf7432ce0c65a5db162a4e07f310a8876` while deliberately retaining:

- `.lvk-preview/` supervisor, preview2 Vite configuration and verification helpers;
- `packages/local-web/src/app/entry/DevAgentation.tsx` and its Bootstrap mount;
- the local-web development dependency and lock entries for `agentation@3.1.2`;
- `tests/agentation/` internal trial fixtures and checks.

These are intentional preview-only differences from main. Exclude them from any
future product integration; do not merge this branch wholesale back into main.
Production builds must exclude the Agentation package, and other development
hostnames must not render it. No endpoint, webhook or external callback is set.
The existing authentication gate still surrounds the mount. Preserve the user's
origin/localStorage; run validation in fresh, isolated browser contexts.

The preview2 configuration keeps `.build/design-preview/instance`, backend
port 18446, Vite port 18444, the dedicated `.vite-design-preview` cache and the
existing public origin. Do not replace them with another preview's paths. The
backend remains the existing SHA-pinned e143-based binary until a separately
prepared managed cutover. Importing main source does not update that binary.

Before this sync, the clean source was
`f8610053323fb06336eb4cd2e376b4cfb1537db0`, retained by the local ref
`refs/preview-backups/local-300-cfb0deb0`. The sync merge can be undone, if later
requested and after checking for newer work, with `git revert -m 1 <sync-merge>`.
Do not use a force reset. This restores source only; never reset/copy the preview
DB, authentication or browser data as a source rollback. Build metadata and
before/after verification are local artifacts under
`.build/design-preview/main-sync-cfb0deb0/`, outside product commits.
