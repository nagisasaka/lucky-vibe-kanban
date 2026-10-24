# Codex child cleanup regression (LOCAL-102)

Base: `eda9ff3f89c337b4010496ccc59a2c6219378ed7`.
This work does not identify the historical source of the vanished PIDs 88877 or
1253930. It addresses a code-supported lifetime failure independently of them.

## Executed lightweight checks

```sh
python3 tests/codex-process-cleanup/reproduce.py
python3 tests/codex-process-cleanup/check_owned_group.py
```

The first reproduces direct-wrapper-only death while a retained stdin keeps the
native child alive. Its timeout/drop/abort labels all emulate that same final
mechanism; they are **not three executed Rust paths**. All fixture processes are
stopped and waited for, and an unrelated process survives the tested cleanup.

The second checks Linux `waitid(WNOWAIT)` with pidfds: normal fake initialize and
wrapper-first exit, unreaped identity anchor, exact native termination, unrelated
process survival, then leader reap. This validates kernel primitives, not the
Rust implementation. The fixture never invokes Codex, a provider, network, or a
live instance. Evidence: `.task-evidence/codex-cleanup/` (workspace-local).

## Executed Rust validation (2026-10-25 JST)

The user released the fixed LOCAL161 wait at 01:34 JST, authorising this workspace
at `CARGO_BUILD_JOBS=1`. Existing target and `/usr/local/bin/sccache` were reused;
`SCCACHE_DIR` matched the canonical `.lvk-shared/cache/sccache` server location.
No cache reset, clean build, unrelated suite, live DB, or provider invocation was
used. Tests used isolated temporary fixtures; final Rust commands set `TMPDIR`
to this workspace's `.task-evidence/codex-cleanup/tmp`.

| Command (all with jobs=1 and sccache) | Final result | Evidence under `.task-evidence/codex-cleanup/rust/` |
| --- | --- | --- |
| `cargo test --locked -p utils --lib process:: -- --nocapture --test-threads=1` | 5 passed | `utils-process-final.log` |
| `cargo test --locked -p executors --lib discovery_cleanup_normal_timeout_drop_abort -- --nocapture` | 1 passed, 4 internal cases | `discovery-final.log` |
| `cargo test --locked -p local-deployment --lib process_host::tests -- --nocapture --test-threads=1` | 10 passed | `process-host-final.log` |

The Rust direct-drop reproduction confirms that actual `AsyncGroupChild` drop
kills the wrapper while its native child survives with retained RPC stdin. This
is separate from the earlier Python mechanism reproduction and from the fixed
production discovery test. Exact fixture processes are terminated afterwards.
An expected Python `KeyboardInterrupt` in the utility log is fixture termination;
the Rust assertions and command both passed.

The discovery test exercises actual discovery owner/RPC code with normal
completion, timeout, dropping the future, and caller task abort. It pins fixture
identities before allowing successful completion, waits for wrapper/native exit
and leader reap, and verifies an unrelated process survives. Future drop covers
the lifetime edge used by WebSocket disconnect, **not an actual WebSocket router
integration test**. Normal completion also awaits the tracked RPC reader.

The process-host natural-exit test now leaves an owned child behind and covers
success evidence, failure evidence, nonzero exit and cancellation. It checks
owned-child exit and unrelated survival. Existing native-interrupt, bounded
control, terminal drain and reconnect tests also pass. No formal Integration or
real provider execution is claimed.

Two failures were retained and fixed before these final runs:

- `discovery.log`: compilation rejected the incorrect `utils` crate name;
  changed to this crate's existing `workspace_utils` alias.
- `process-host.log`: 9 passed, 1 failed because cleanup waited four seconds even
  after all processes exited. Grace now ends when pinned processes exit, while
  retaining the final group rescan and reap checks. The unchanged drain deadline
  passes at about 16 ms for both success and failure in the final run.

Scoped `rustfmt --check`, `git diff --check`, and offline locked Cargo metadata
also passed. Separate `cargo check` and unrelated full suites were omitted because
the focused tests compile the changed production code and its affected consumer.
The change is ready for the upstream independent review, not approved for merge
or deployment. Final source SHA, timestamps and file/log hashes are recorded in
`.task-evidence/codex-cleanup/completion.json` and the completion report.

## Ownership and limits

The caller's cancellation guard notifies a discovery owner task; the owner awaits
process cleanup and the tracked RPC reader even if its caller disappears. Cleanup
failures are logged without arguments or environment contents. A task panic is
converted into an error before cleanup. This is not an external supervisor and
cannot survive an OS kill or destruction of the entire Tokio runtime.

On Linux, the host observes child exit without reaping, retains the leader's
PID/start/PGID/session identity and signals only pidfds captured under that anchor.
Natural child exit no longer bypasses group cleanup. Existing descendant capture
also covers descendants present at cleanup that have created separate sessions;
it does not prove containment of descendants that escaped and were reparented
before observation. Non-Linux keeps the existing group cleanup mechanism and natural-exit behavior;
no post-reap PGID signalling is added there. It requires separate platform validation.

No self_restart idle-guard relaxation, live signals, DB copy, configuration,
prepared bundle, plugin/Skill, Wiki, main merge, or deployment change is included.
The prepared eda9 bundle does not contain this change and remains on hold.
