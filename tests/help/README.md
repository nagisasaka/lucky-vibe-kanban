# Interactive help fixture

Run from the repository root after installing dependencies:

```sh
pnpm --filter @vibe/web-core exec vitest run src/shared/components/InteractiveHelp.test.tsx
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/help/reading.mjs
```

The browser fixture mounts the real Help UI and transport with synthetic API
responses. It checks starting, progress, answer, continuation, cancellation,
provider failure, missing source and conversation deletion, plus the existing
Workspace link for native input and approval handling. It checks light/dark,
wide/narrow layouts and writes screenshots to ignored `.build/help-review/`.
It does not start a backend or agent, use a database, validate real provider or
server authentication, or prove the bundled distribution source matches a live
binary. SSR tests additionally distinguish all existing runtime statuses.

## LOCAL-375 continuation verification (2026-11-11)

The continuation independently built the current dirty implementation with
`node scripts/dev-cargo.mjs build -p server --bin server -p local-deployment --bin agent-process-host -j 2`
(exit 0). Its embedded revision was `2e5a7fa7db4fc6e86388143ff8265066799e6d55`,
`kind=development`, `dirty=true`. The reference repository was fetched anonymously
from the public upstream at that revision using `deploy/server/bundle-source.sh`;
it did not contain the uncommitted help implementation.

An isolated `LVK_INSTANCE_DIR`, ephemeral loopback port and existing Codex login
were used for the real HTTP help service. Standard local Origin and
`x-lvk-csrf: local` headers preserved the existing authentication contract.
Inherited deployment URL/port settings were removed only from test child
processes. The running LVK instance and its authentication were not changed.

Observed results:

- General usage question and follow-up both reached `succeeded`, with the same
  native provider session. Answers cited source/Wiki paths, lines and revision.
- The follow-up quoted the message of `b5563ce25a291a2867a6eaa67e55adb05f6df183`,
  distinguished diff facts from interpretation, and found the real PR guidance
  in `CONTRIBUTORS.md` rather than inventing its absence.
- A separate run reported `running` when cancelled and reached `cancelled`;
  cancellation happened during native initialization, not proven during answer
  generation. Conversation deletion succeeded after terminal status.
- Project-free read/list/grep/log/diff returned HTTP 200 and `success=true` through
  the actual libgit2 HTTP read path under the current nonroot user. This is not
  the final image's UID/GID ownership test.
- Missing source, mismatched source revision and empty questions returned 400.
  A nonexistent profile was accepted as a launch request but its run immediately
  reported `failed/startup_failed`, not a successful answer.
- A separate empty task-owned `CODEX_HOME` reported `Not logged in` (CLI exit 1)
  without changing the real login or HOME. Its help run reached `failed` and
  deletion succeeded. The runtime exposed only `provider executor reported
  failure`, so a specific provider authentication diagnostic was not verified.
  The CLI also warned that creating PATH alias helpers under the temporary
  directory was refused; that refused helper creation was not retried or
  bypassed. The failed run alone does not isolate an authentication root cause.
- Source `git status --porcelain` stayed empty; the isolated database retained
  zero Projects and zero nonterminal AgentRuns. All test server processes were
  stopped and waited; task-owned server/provider/host process checks were empty.

Initial harness attempts did not dispatch provider questions: inherited
`BACKEND_PORT` caused `AddrInUse` (server exit 1), inherited public deployment URL
caused an invalid Host response (that test process was stopped), and omitted
local CSRF headers caused 403. The client then supplied the normal headers;
no CSRF or authentication checks were disabled.

Redacted answers, run IDs/statuses, binary hashes and the manual probe scripts
are retained locally in ignored `.build/help-review/local375-runtime-summary.json`,
`local375-probe.py` and `local375-authless.py`. The scripts take a task-owned
scratch-directory argument containing a source checkout produced by the helper;
run the ordinary probe first and authless probe second. They create isolated
state, stop/wait their servers and write scratch JSON results. The caller must
remove its scratch directory after checking that no test process remains.
These probes use existing provider access and can incur provider usage; they
are manual integration checks, not part of the synthetic browser fixture.

`pnpm --filter @vibe/web-core exec vitest run src/shared/components/InteractiveHelp.test.tsx`
was independently rerun in the continuation: 1 file, 13 tests passed, exit 0.

Completed Docker image/nonroot UID 10001/libgit2/volume-update validation, real
remote MCP OAuth, and browser-plus-real-provider integration remain separate
unverified scopes. Earlier Run reports of Rust/Clippy/type/lint tests are not
new independent passes in this continuation.
