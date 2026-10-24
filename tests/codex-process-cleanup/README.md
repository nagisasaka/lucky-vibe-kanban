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

## Pending Rust validation — capacity release required

Do not start these until LOCAL161's heavy slot is explicitly released. Recheck
capacity, use existing target/cache and `CARGO_BUILD_JOBS=2`, and follow AGENTS.md
sccache instructions. No clean build is needed. Run sequentially:

```sh
cargo test -p utils owned_group -- --nocapture
cargo test -p utils cancellation_cleans_up_orphaned_session_children_not_other_processes -- --nocapture
cargo test -p executors discovery_cleanup_normal_timeout_drop_abort -- --nocapture
cargo test -p local-deployment process_host::tests -- --nocapture
cargo check -p executors -p local-deployment
```

The new discovery test uses the actual discovery process owner and RPC reader,
with a Python wrapper/native fake. It covers normal completion, timeout, dropping
the discovery future (the lifetime edge used by WebSocket disconnect), and abort.
It does not exercise a real WebSocket router or formal Integration end to end.
The process-host tests must preserve existing success/cancel/terminal semantics.
Rust compilation and these tests have **not yet run**; this is not review-ready.

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
before observation. Non-Linux keeps the existing group cleanup mechanism and
requires separate platform validation.

No self_restart idle-guard relaxation, live signals, DB copy, configuration,
prepared bundle, plugin/Skill, Wiki, main merge, or deployment change is included.
The prepared eda9 bundle does not contain this change and remains on hold.
