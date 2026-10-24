"""Developer artifact cutover; builds and migration compatibility belong to callers."""

import argparse
import hashlib
import json
import os
from pathlib import Path as P
import select
import shutil
import signal
import subprocess
import sys
import time
import uuid
import start_support as s

DEV = P(__file__).resolve().parent.parent
TERMINAL = {"succeeded", "failed", "interrupted"}
PAYLOAD = {"server", "agent-process-host", "vibe-kanban-mcp"}


def path(value):
    p = P(value).absolute()
    s.need(p.resolve() == p, "Use a canonical path without symlinks")
    return p


def configure(instance):
    global DEV, CONFIG
    DEV = path(instance)
    s.need(DEV.is_dir() and DEV.stat().st_uid == os.getuid(), "Instance owner mismatch")
    filename = DEV / "launch.json"
    s.need(
        not filename.is_symlink() and not (DEV / "data").is_symlink(),
        "Instance alias refused",
    )
    s.need(filename.stat().st_mode & 0o077 == 0, "Launch settings must be owner-only")
    CONFIG = s.readjson(filename)
    s.need(
        set(CONFIG) == {"port", "environment"},
        "launch.json requires port and environment",
    )
    s.need(
        isinstance(CONFIG["port"], int) and 1024 <= CONFIG["port"] <= 65535,
        "Invalid port",
    )
    s.need(
        isinstance(CONFIG["environment"], dict)
        and all(
            isinstance(k, str) and isinstance(v, str)
            for k, v in CONFIG["environment"].items()
        ),
        "Invalid launch environment",
    )
    for name in ["launcher.py", "start_binary.py", "start_support.py", "start.sh"]:
        helper = DEV / "switch" / name
        s.need(
            helper.is_file()
            and helper.resolve() == helper
            and not helper.stat().st_mode & 0o022,
            "Fixed launcher kit missing, aliased or writable by others",
        )
    s.configure(DEV, CONFIG["port"])
    return filename


def current():
    s.need(not (DEV / "current.json").is_symlink(), "Current selection alias refused")
    selection = s.readjson(DEV / "current.json")
    release = path(selection["release"])
    s.need(
        release.parent == DEV / "releases" and release.is_dir(),
        "Current release outside instance",
    )
    return selection


def inspect(a):
    bundle = path(a.bundle)
    s.need(bundle.is_dir(), "Bundle missing")
    s.need(
        not any(p.is_symlink() for p in bundle.rglob("*")), "Bundle symlinks refused"
    )
    manifest = s.readjson(bundle / "bundle.json")
    s.need(
        manifest.get("format") == 1 and set(manifest["files"]) == PAYLOAD,
        "Bundle requires server, agent-process-host and vibe-kanban-mcp",
    )
    s.need(
        set(p.name for p in bundle.iterdir()) == PAYLOAD | {"bundle.json"},
        "Unexpected bundle payload",
    )
    for name, digest in manifest["files"].items():
        f = bundle / name
        s.need(f.is_file() and os.access(f, os.X_OK), "Executable missing")
        s.need(not f.stat().st_mode & 0o022, "Writable executable refused")
        s.need(s.digest(f) == digest, "Bundle executable hash mismatch")
    return {"bundle": str(bundle), "manifest": manifest}


def probe(release, manifest):
    # This command exits before DB/auth/listener initialisation. It verifies actual
    # embedded bytes, including SoundAssets and ScriptAssets, not manifest claims.
    result = subprocess.run(
        [str(release / "server"), "runtime-payload"],
        cwd=DEV,
        env={"PATH": "/usr/bin:/bin"},
        capture_output=True,
        timeout=20,
    )
    s.need(
        result.returncode == 0 and len(result.stdout) <= 4 * 1024 * 1024,
        "Candidate payload probe failed",
    )
    payload = json.loads(result.stdout)
    s.need(payload["embedded"] is True, "Candidate assets are not embedded")
    s.need(payload["assets"] == manifest["assets"], "Embedded payload hash mismatch")
    s.need(
        "frontend/index.html" in payload["assets"]
        and any(k.startswith("sounds/") for k in payload["assets"])
        and any(k.startswith("scripts/") for k in payload["assets"]),
        "Runtime payload incomplete",
    )
    for name in ["agent-process-host", "vibe-kanban-mcp"]:
        result = subprocess.run(
            [str(release / name), "runtime-payload"],
            cwd=DEV,
            env={"PATH": "/usr/bin:/bin"},
            capture_output=True,
            timeout=20,
        )
        s.need(
            result.returncode == 0
            and json.loads(result.stdout) == {"component": name, "protocol": 1},
            "Runtime executable probe failed",
        )
    return payload


def observation():
    s.need(
        s.listeners(),
        "Expected running instance; use the fixed launcher for initial startup",
    )
    pid = int((DEV / "server.pid").read_text())
    want = s.identity(pid)
    selection = current()
    release = P(selection["release"])
    s.need(
        want["exe"] == str(release / "server") and want["cwd"] == str(DEV),
        "PID differs from current selection",
    )
    s.need(s.owns_port(pid), "Recorded PID does not own configured loopback port")
    s.need(
        s.digest(f"/proc/{pid}/exe") == selection["manifest"]["files"]["server"],
        "Running executable hash differs",
    )
    env = dict(
        x.split(b"=", 1)
        for x in P(f"/proc/{pid}/environ").read_bytes().split(b"\0")
        if b"=" in x
    )
    s.need(env.get(b"LVK_INSTANCE_DIR") == str(DEV).encode(), "PID instance mismatch")
    s.need(
        env.get(b"LVK_LAUNCH_CONFIG_SHA256") == s.digest(DEV / "launch.json").encode(),
        "Launch settings differ from running process",
    )
    s.need(
        env.get(b"VIBE_KANBAN_AGENT_PROCESS_HOST")
        == str(release / "agent-process-host").encode(),
        "PID agent host mismatch",
    )
    return want


def snapshot():
    return {
        "identity": observation(),
        "instance": str(DEV),
        "data": str(DEV / "data"),
        "port": CONFIG["port"],
        "current": current(),
        "launch_hash": s.digest(DEV / "launch.json"),
    }


def preflight(a, *, dry_run=False):
    if not dry_run:
        s.need(a.no_new_work, "Coordinate idle work externally and pass --no-new-work")
    s.need(not P("/run/lvk-maintenance/active").exists(), "Host maintenance active")
    s.need(
        not (DEV / "run/supervisor.sock").exists(),
        "Supervisor ownership requires operator recovery",
    )
    selected = inspect(a)
    selected["snapshot"] = snapshot()
    s.resources()
    s.idle(selected["snapshot"]["identity"]["pid"])
    s.need(
        not any(p.is_symlink() for p in (DEV / "data").rglob("*")),
        "Data symlinks refused",
    )
    size = sum(p.stat().st_size for p in (DEV / "data").rglob("*") if p.is_file())
    artifact_size = sum((P(selected["bundle"]) / k).stat().st_size for k in PAYLOAD)
    selected["backup"] = {
        "scope": ["data/", "launch.json", "current.json", "switch/"],
        "data_bytes": size,
        "artifact_bytes": artifact_size,
        "required_free_bytes": size * 2 + artifact_size + 1024**3,
    }
    s.need(
        shutil.disk_usage(DEV).free > selected["backup"]["required_free_bytes"],
        "Insufficient backup space",
    )
    return selected


def preserve(selected, op):
    release = DEV / "releases" / op.name
    release.mkdir(mode=0o700)
    for name, digest in selected["manifest"]["files"].items():
        shutil.copy2(P(selected["bundle"]) / name, release / name)
        s.need(s.digest(release / name) == digest, "Candidate changed while copying")
        (release / name).chmod(0o500)
    probe(release, selected["manifest"])
    selected["release"] = str(release)
    # From here onward only copied payloads are consulted.
    return {"release": str(release), "manifest": selected["manifest"]}


def verify_release(selection):
    for name, digest in selection["manifest"]["files"].items():
        f = P(selection["release"]) / name
        s.need(not f.is_symlink() and s.digest(f) == digest, "Release changed")


def sync_tree(root):
    for f in root.rglob("*"):
        if f.is_file():
            with f.open("rb") as stream:
                os.fsync(stream.fileno())
    for d in sorted(
        [root.parent, root] + [p for p in root.rglob("*") if p.is_dir()],
        key=lambda p: len(p.parts),
        reverse=True,
    ):
        fd = os.open(d, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)


def prune_success(op, previous, selected):
    # Only successful transactions can remove superseded large material. Receipts
    # and logs stay; failure admission fences growth until explicit operator recovery.
    keep = {previous["release"], selected["release"]}
    for release in (DEV / "releases").iterdir():
        s.need(not release.is_symlink(), "Release alias refused")
        if str(release) not in keep:
            shutil.rmtree(release)
    for prior in (DEV / "switch-operations").iterdir():
        if prior == op or not (prior / "result.json").exists():
            continue
        if s.readjson(prior / "result.json")["status"] == "succeeded":
            for name in ["data", "settings"]:
                if (prior / name).exists():
                    shutil.rmtree(prior / name)


def write_state(op, state):
    state["updated_at"] = time.time()
    s.atomic(op / "result.json", (json.dumps(state, indent=2) + "\n").encode())


def status(request_id, owner):
    op = DEV / "switch-operations" / str(uuid.UUID(request_id))
    s.need(
        (op / "request.json").is_file(),
        "Incomplete operation admission; operator inspection required",
    )
    request = s.readjson(op / "request.json")
    s.need(request["owner"] == owner, "Operation owner mismatch")
    result = (
        s.readjson(op / "result.json")
        if (op / "result.json").exists()
        else {
            "operation_id": op.name,
            "status": "accepted",
            "stage": "admission",
            "result_path": str(op / "result.json"),
        }
    )
    if result["status"] not in TERMINAL:
        # A vanished worker is not success and must never cause automatic replay.
        with (DEV / "start.lock").open("a") as lock:
            try:
                s.fcntl.flock(lock, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
            except BlockingIOError:
                return result
            result = (
                s.readjson(op / "result.json")
                if (op / "result.json").exists()
                else result
            )
            if result["status"] not in TERMINAL:
                result.update(
                    status="interrupted",
                    reason="Worker no longer holds lock; inspect preserved state before recovery",
                )
                write_state(op, result)
    return result


def health(proc, selected):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        s.need(proc.poll() is None, "New server exited; inspect protected server log")
        try:
            s.need(
                s.get("/api/health").get("success") is True, "Health endpoint refused"
            )
            s.need(s.owns_port(proc.pid), "New process does not own configured port")
            s.need(s.identity(proc.pid)["cwd"] == str(DEV), "Launched cwd mismatch")
            s.need(
                s.digest(f"/proc/{proc.pid}/exe")
                == selected["manifest"]["files"]["server"],
                "Launched binary hash mismatch",
            )
            verify_release(selected)
            for name, digest in selected["manifest"]["assets"].items():
                if name.startswith("frontend/") and (
                    name == "frontend/index.html" or name.endswith((".js", ".css"))
                ):
                    s.need(
                        hashlib.sha256(
                            s.get_bytes("/" + name.removeprefix("frontend/"))
                        ).hexdigest()
                        == digest,
                        "Served frontend differs from bundle",
                    )
            return s.get("/api/info")["data"].get("build_info", {})
        except (ConnectionError, TimeoutError, s.urllib.error.URLError):
            time.sleep(0.2)
    raise s.Refuse("Health timeout")


def execute(op, request, config_path):
    a = argparse.Namespace(**request["arguments"])
    state = s.readjson(op / "result.json")

    def stage(name):
        state.update(status="running", stage=name)
        write_state(op, state)

    def interrupted(signum, frame):
        raise s.Refuse("Worker termination requested")

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        stage("preflight")
        selected = preflight(a)
        expected = request.get("snapshot")
        if expected:
            s.need(
                selected["snapshot"] == expected,
                "Running instance changed after admission",
            )
        s.save(op / "selection.json", selected)
        previous = current()
        verify_release(previous)
        selected = preserve(selected, op)
        sync_tree(P(selected["release"]))
        s.save(op / "launch-selection.json", selected)
        stage("stopping")
        s.need(snapshot() == expected, "Launch settings or running identity changed")
        verify_release(selected)
        verify_release(previous)
        s.resources()
        s.idle(expected["identity"]["pid"])
        s.stop(expected["identity"], previous["manifest"]["files"]["server"])
        stage("backup")
        s.stopped()
        (op / "settings").mkdir(mode=0o700)
        for name in ["launch.json", "current.json"]:
            shutil.copy2(DEV / name, op / "settings" / name)
        shutil.copytree(DEV / "switch", op / "settings/switch")
        manifest = s.backup_data(DEV / "data", op / "data")
        s.verify_tree(op / "data", manifest)
        s.save(op / "data-manifest.json", manifest)
        sync_tree(op)
        s.atomic(
            op / "backup-complete.json",
            json.dumps(
                {"manifest_sha256": s.digest(op / "data-manifest.json")}
            ).encode(),
        )
        stage("switching")
        s.stopped()
        s.need(
            s.digest(DEV / "launch.json") == expected["launch_hash"]
            and current() == previous,
            "Launch settings changed after backup",
        )
        verify_release(selected)
        s.atomic(DEV / "current.json", json.dumps(selected).encode())
        stage("starting")
        with (op / "server.log").open("xb") as log:
            proc = subprocess.Popen(
                ["/usr/bin/python3", str(DEV / "switch/launcher.py"), str(DEV)],
                cwd=DEV,
                env={"PATH": "/usr/local/bin:/usr/bin:/bin"},
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
        s.save(op / "launch.json", {"pid": proc.pid})
        stage("health")
        info = health(proc, selected)
        with s.ro(DEV / "data/db.v2.sqlite") as db:
            s.need(
                db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
                and not db.execute("PRAGMA foreign_key_check").fetchall(),
                "Post-start DB integrity failure",
            )
        stage("retention")
        prune_success(op, previous, selected)
        state.update(
            status="succeeded",
            stage="complete",
            process=s.identity(proc.pid),
            build_info=info,
            checks={
                "health": True,
                "artifact_identity": True,
                "frontend_assets": True,
                "db_integrity": True,
                "auth0_ui_mcp": "not_verified",
            },
        )
    except BaseException as error:
        state.update(
            status="failed",
            reason=str(error) if isinstance(error, s.Refuse) else type(error).__name__,
        )
    finally:
        write_state(op, state)
    return state


def submit(a, config_path):
    args = {
        k: v
        for k, v in vars(a).items()
        if k
        not in [
            "instance",
            "snapshot_stdin",
            "asynchronous",
            "status",
            "dry_run",
            "request_id",
            "owner",
            "worker",
            "lock_fd",
            "admission_fd",
        ]
    }
    request = {"owner": a.owner, "arguments": args}
    root = DEV / "switch-operations"
    root.mkdir(mode=0o700, exist_ok=True)
    op = root / str(uuid.UUID(a.request_id))
    if (op / "request.json").exists():
        s.need(
            {
                k: v
                for k, v in s.readjson(op / "request.json").items()
                if k != "snapshot"
            }
            == request,
            "Request ID reused with different input",
        )
        return status(a.request_id, a.owner)
    with (DEV / "start.lock").open("a") as lock:
        try:
            s.fcntl.flock(lock, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
        except BlockingIOError:
            raise s.Refuse("Another switch is active; query its existing operation")
        if op.exists():
            s.need(
                {
                    k: v
                    for k, v in s.readjson(op / "request.json").items()
                    if k != "snapshot"
                }
                == request,
                "Request ID reused with different input",
            )
            return s.readjson(op / "result.json")
        for previous in root.iterdir():
            if previous.is_dir() and not previous.name.startswith(".pending-"):
                s.need(
                    (previous / "result.json").exists(),
                    "Incomplete previous operation; operator inspection required",
                )
                prior = s.readjson(previous / "result.json")
                s.need(
                    prior["status"] == "succeeded",
                    "Unsettled previous operation; inspect before operator recovery",
                )
        request["snapshot"] = json.load(sys.stdin) if a.snapshot_stdin else snapshot()
        pending = root / (".pending-" + uuid.uuid4().hex)
        pending.mkdir(mode=0o700)
        s.atomic(pending / "request.json", (json.dumps(request) + "\n").encode())
        state = {
            "operation_id": op.name,
            "status": "accepted",
            "stage": "accepted",
            "result_path": str(op / "result.json"),
        }
        write_state(pending, state)
        os.rename(pending, op)
        directory = os.open(root, os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        if a.asynchronous:
            # A dedicated worker owns the ENTIRE transaction, not just server startup.
            # The lock descriptor is passed to that worker only (never the new server).
            admission_fd = os.pidfd_open(os.getpid())
            with (op / "worker.log").open("xb") as log:
                worker = subprocess.Popen(
                    [
                        sys.executable,
                        str(P(__file__).resolve()),
                        "--instance",
                        str(DEV),
                        "--worker",
                        op.name,
                        "--lock-fd",
                        str(lock.fileno()),
                        "--admission-fd",
                        str(admission_fd),
                    ],
                    stdin=subprocess.DEVNULL,
                    stdout=log,
                    stderr=log,
                    start_new_session=True,
                    pass_fds=(lock.fileno(), admission_fd),
                    env={"PATH": "/usr/local/bin:/usr/bin:/bin", "LANG": "C.UTF-8"},
                )
            os.close(admission_fd)
            state["worker_pid"] = worker.pid
            # Worker owns subsequent state writes. Do not overwrite its progress here.
            return state
        return execute(op, request, config_path)


def main():
    p = argparse.ArgumentParser(
        description="Switch externally tested embedded dev bundles. No automatic rollback."
    )
    p.add_argument(
        "--instance",
        default=str(P(__file__).resolve().parent.parent),
        help="Defaults to the instance containing this installed switch kit",
    )
    p.add_argument("--bundle")
    p.add_argument("--no-new-work", action="store_true")
    p.add_argument("--request-id")
    p.add_argument("--owner", default="operator")
    p.add_argument("--async", dest="asynchronous", action="store_true")
    p.add_argument("--status", action="store_true")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--snapshot-stdin", action="store_true", help=argparse.SUPPRESS)
    p.add_argument("--worker", help=argparse.SUPPRESS)
    p.add_argument("--lock-fd", type=int, help=argparse.SUPPRESS)
    p.add_argument("--admission-fd", type=int, help=argparse.SUPPRESS)
    a = p.parse_args()
    os.umask(0o077)
    config = configure(a.instance)
    if a.worker:
        s.need(
            a.lock_fd is not None
            and os.path.samefile(f"/proc/self/fd/{a.lock_fd}", DEV / "start.lock"),
            "Missing inherited switch lock",
        )
        s.fcntl.flock(a.lock_fd, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
        s.need(
            a.admission_fd is not None
            and bool(select.select([a.admission_fd], [], [], 30)[0]),
            "Admission process did not exit",
        )
        os.close(a.admission_fd)
        op = DEV / "switch-operations" / str(uuid.UUID(a.worker))
        s.save(op / "worker.json", s.identity(os.getpid()))
        result = execute(op, s.readjson(op / "request.json"), config)
    elif a.status:
        result = status(a.request_id, a.owner)
    elif a.dry_run:
        s.need(a.bundle, "--bundle required")
        result = preflight(a, dry_run=True)
    else:
        s.need(a.bundle and a.request_id, "--bundle and --request-id required")
        result = submit(a, config)
    print(json.dumps(result), flush=True)
    return 2 if result.get("status") in {"failed", "interrupted"} else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(
            json.dumps(
                {
                    "status": "refused",
                    "reason": (
                        str(error)
                        if isinstance(error, s.Refuse)
                        else type(error).__name__
                    ),
                }
            ),
            flush=True,
        )
        sys.exit(2)
