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
