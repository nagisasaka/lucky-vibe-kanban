"""Reusable development binary selection. Inspection never executes the candidate."""

import argparse, ast, hashlib, json, mmap, os, pathlib, shutil, subprocess, sys, time, uuid, signal
import start_support as s

P = pathlib.Path
DEV = P(__file__).resolve().parent


def path(value):
    p = P(value).absolute()
    s.need(p.resolve() == p, "Use a canonical path without symlinks")
    return p


def frontend_hash(root):
    s.need((root / "index.html").is_file(), "Built frontend index missing")
    files = {}
    for p in sorted(root.rglob("*")):
        s.need(not p.is_symlink(), "Frontend symlink refused")
        if p.is_file():
            files[p.relative_to(root).as_posix()] = s.digest(p)
    tree = hashlib.sha256(
        "".join(f"{h}  {n}\n" for n, h in files.items()).encode()
    ).hexdigest()
    return tree, files


def embedded_path(binary, frontend):
    with binary.open("rb") as f:
        with mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ) as m:
            s.need(
                m.find(str(frontend).encode()) >= 0,
                "Debug frontend absolute path not found in binary; inspect this build separately",
            )


def inspect(a):
    binary = path(a.binary)
    source = path(a.source) if a.source else binary.parent.parent.parent
    host = (
        path(a.agent_host) if a.agent_host else binary.with_name("agent-process-host")
    )
    front = path(a.frontend) if a.frontend else source / "packages/local-web/dist"
    s.need(
        source.is_dir() and source.resolve() == source, "Source root missing or aliased"
    )
    for p in [binary, host]:
        s.need(p.is_file() and os.access(p, os.X_OK), "Executable missing")
    hashes = {str(p): s.digest(p) for p in [binary, host]}
    tree, files = frontend_hash(front)
    if getattr(a, "frontend_mode", "external") == "external":
        embedded_path(binary, front)
    else:
        s.need(a.manifest, "Embedded frontend requires a hash-bound build manifest")
    revision = None
    origin = "unknown (not inferred from checkout HEAD)"
    if a.manifest:
        m = json.loads(path(a.manifest).read_text())
        entries = {x["path"]: x for x in m["artifacts"]}
        for p in [binary, host]:
            s.need(
                entries.get(str(p), {}).get("sha256") == hashes[str(p)],
                "Manifest executable hash mismatch",
            )
        f = m["frontend"]
        s.need(
            f["path"] == str(front)
            and f["tree_sha256"] == tree
            and f["files"] == files,
            "Manifest frontend mismatch",
        )
        revision = m.get("source_sha")
        origin = (
            "hash-verified build manifest (runtime build identity checked after launch)"
        )
    if a.sha:
        s.need(
            revision is not None and revision == a.sha,
            "Requested SHA is unavailable or differs from build manifest",
        )
    return {
        "binary": str(binary),
        "agent_host": str(host),
        "source": str(source),
        "frontend": str(front),
        "binary_hashes": hashes,
        "frontend_tree_sha256": tree,
        "frontend_mode": getattr(a, "frontend_mode", "external"),
        "recorded_build_sha": revision,
        "sha_evidence": origin,
    }


def cleanup_check(selection, retain):
    config = s.readjson(DEV / "data/config.json")
    bases = [DEV / "runtime/worktrees"]
    if config.get("workspace_dir"):
        bases.append(P(config["workspace_dir"]).resolve() / ".vibe-kanban-workspaces")
    affected = []
    for name in ["source", "binary", "agent_host", "frontend"]:
        p = P(selection[name])
        for base in bases:
            if p.is_relative_to(base):
                affected.append(base / p.relative_to(base).parts[0])
    if affected:
        s.need(
            retain,
            "Candidate depends on cleanup-managed worktree; explicitly use --retain-worktree and preserve it while running",
        )
        with s.ro(DEV / "data/db.v2.sqlite") as c:
            for root in set(affected):
                s.need(
                    c.execute(
                        "SELECT count(*) FROM workspaces WHERE container_ref=?",
                        (str(root),),
                    ).fetchone()[0]
                    == 1,
                    "Cleanup-managed worktree registration missing",
                )
    # Require the setting in the actual env.update expression, independently of quote style.
    dictionaries = [
        n.args[0]
        for n in ast.walk(ast.parse((DEV / "run.py").read_text()))
        if isinstance(n, ast.Call)
        and isinstance(n.func, ast.Attribute)
        and isinstance(n.func.value, ast.Name)
        and n.func.value.id == "env"
        and n.func.attr == "update"
        and len(n.args) == 1
        and isinstance(n.args[0], ast.Dict)
    ]
    s.need(
        any(
            any(
                isinstance(k, ast.Constant)
                and k.value == "DISABLE_WORKTREE_CLEANUP"
                and isinstance(v, ast.Constant)
                and v.value == "1"
                for k, v in zip(n.keys, n.values)
            )
            for n in dictionaries
        ),
        "Existing cleanup disable setting missing",
    )
    s.need(
        P(selection["source"]).is_relative_to(DEV / "builds") or retain,
        "Source outside retained instance builds requires --retain-worktree and external retention coordination",
    )
    return [str(p) for p in sorted(set(affected))]


def new_run(old, selection):
    tree = ast.parse(old)
    counts = {"source": 0, "host": 0, "exec": 0}

    class Change(ast.NodeTransformer):
        def visit_Assign(self, node):
            if any(isinstance(x, ast.Name) and x.id == "source" for x in node.targets):
                counts["source"] += 1
                node.value = ast.parse(
                    "Path(" + repr(selection["source"]) + ")", mode="eval"
                ).body
            return self.generic_visit(node)

        def visit_Dict(self, node):
            for i, k in enumerate(node.keys):
                if (
                    isinstance(k, ast.Constant)
                    and k.value == "VIBE_KANBAN_AGENT_PROCESS_HOST"
                ):
                    counts["host"] += 1
                    node.values[i] = ast.Constant(selection["agent_host"])
            return self.generic_visit(node)

        def visit_Call(self, node):
            if (
                isinstance(node.func, ast.Attribute)
                and isinstance(node.func.value, ast.Name)
                and node.func.value.id == "os"
                and node.func.attr == "execve"
            ):
                counts["exec"] += 1
                s.need(
                    len(node.args) == 3
                    and isinstance(node.args[2], ast.Name)
                    and node.args[2].id == "env",
                    "Unexpected execve template",
                )
                node.args[0] = ast.Constant(selection["binary"])
                node.args[1] = ast.List(
                    elts=[ast.Constant(selection["binary"])], ctx=ast.Load()
                )
            return self.generic_visit(node)

    tree = Change().visit(tree)
    tree.body = [
        n
        for n in tree.body
        if not (
            isinstance(n, ast.Assign)
            and any(
                isinstance(t, ast.Name) and t.id == "lvk_frontend_dependency"
                for t in n.targets
            )
        )
    ]
    tree.body.insert(
        0,
        ast.Assign(
            targets=[ast.Name(id="lvk_frontend_dependency", ctx=ast.Store())],
            value=ast.Constant(selection["frontend"]),
        ),
    )
    s.need(counts == {"source": 1, "host": 1, "exec": 1}, "Unsupported run.py layout")
    out = ast.unparse(ast.fix_missing_locations(tree)) + "\n"
    compile(out, "run.py", "exec")
    return out.encode()


def old_paths():
    tree = ast.parse((DEV / "run.py").read_text())
    source = None
    host = None
    binary = None
    front = None
    for n in ast.walk(tree):
        if isinstance(n, ast.Assign):
            for t in n.targets:
                if isinstance(t, ast.Name) and t.id == "source":
                    source = P(ast.literal_eval(n.value.args[0]))
                if isinstance(t, ast.Name) and t.id == "lvk_frontend_dependency":
                    front = P(ast.literal_eval(n.value))
        if isinstance(n, ast.Dict):
            for k, v in zip(n.keys, n.values):
                if (
                    isinstance(k, ast.Constant)
                    and k.value == "VIBE_KANBAN_AGENT_PROCESS_HOST"
                    and isinstance(v, ast.Constant)
                ):
                    host = P(v.value)
        if (
            isinstance(n, ast.Call)
            and isinstance(n.func, ast.Attribute)
            and n.func.attr == "execve"
            and isinstance(n.args[0], ast.Constant)
        ):
            binary = P(n.args[0].value)
    s.need(source is not None, "Cannot determine old source")
    return (
        source,
        binary or source / "target/debug/server",
        host or source / "target/debug/agent-process-host",
        front or source / "packages/local-web/dist",
    )


# The server owns migrations. This module never reads migration SQL/history.
TERMINAL = {"succeeded", "failed", "interrupted"}


def configure(filename):
    global DEV, CONFIG
    filename = path(filename)
    CONFIG = s.readjson(filename)
    s.need(
        set(CONFIG) == {"instance", "port"}, "Config requires only instance and port"
    )
    DEV = path(CONFIG["instance"])
    s.need(DEV.is_dir() and 1024 <= CONFIG["port"] <= 65535, "Invalid instance/port")
    s.need(os.stat(DEV).st_uid == os.getuid(), "Instance owner mismatch")
    s.need(
        not (DEV / "data").is_symlink() and not (DEV / "run.py").is_symlink(),
        "Instance alias refused",
    )
    s.configure(DEV, CONFIG["port"])
    return filename


def observation():
    s.need(
        s.listeners(),
        "Expected running instance; use its existing initial startup procedure",
    )
    pid = int((DEV / "server.pid").read_text())
    want = s.identity(pid)
    s.need(s.owns_port(pid), "Recorded PID does not own configured loopback port")
    old = old_paths()
    s.need(
        want["exe"] == str(old[1]) and want["cwd"] == str(old[0]),
        "PID differs from launch configuration",
    )
    s.need(
        s.digest(f"/proc/{pid}/exe") == s.digest(old[1]),
        "Running executable differs from disk",
    )
    # An explicit instance identity is mandatory, even when the port/PID happen to match.
    env = dict(
        x.split(b"=", 1)
        for x in P(f"/proc/{pid}/environ").read_bytes().split(b"\0")
        if b"=" in x
    )
    s.need(env.get(b"LVK_INSTANCE_DIR") == str(DEV).encode(), "PID instance mismatch")
    s.need(
        env.get(b"DISABLE_WORKTREE_CLEANUP") == b"1",
        "Running cleanup retention setting missing",
    )
    return want


def preflight(a):
    s.need(a.no_new_work, "Coordinate idle work externally and pass --no-new-work")
    s.need(not P("/run/lvk-maintenance/active").exists(), "Host maintenance active")
    s.need(
        not (DEV / "run/supervisor.sock").exists(),
        "Supervisor ownership requires operator recovery",
    )
    selection = inspect(a)
    selection["retain_paths"] = cleanup_check(selection, a.retain_worktree)
    selection["current_process"] = observation()
    selection["old_paths"] = dict(
        zip(["source", "binary", "agent_host", "frontend"], map(str, old_paths()))
    )
    selection["old_retain_paths"] = cleanup_check(selection["old_paths"], True)
    selection["old_hashes"] = {
        k: s.digest(selection["old_paths"][k]) for k in ["binary", "agent_host"]
    }
    selection["old_frontend_hash"] = frontend_hash(
        P(selection["old_paths"]["frontend"])
    )[0]
    s.resources()
    s.idle(DEV / "data/db.v2.sqlite")
    s.pre_stop_writers(selection["current_process"]["pid"])
    s.need(
        not any(p.is_symlink() for p in (DEV / "data").rglob("*")),
        "Data symlinks refused",
    )
    size = sum(p.stat().st_size for p in (DEV / "data").rglob("*") if p.is_file())
    artifact_size = sum(
        P(selection[k]).stat().st_size + P(selection["old_paths"][k]).stat().st_size
        for k in ["binary", "agent_host"]
    )
    artifact_size += sum(
        p.stat().st_size
        for root in [P(selection["frontend"]), P(selection["old_paths"]["frontend"])]
        for p in root.rglob("*")
        if p.is_file()
    )
    selection["backup"] = {
        "scope": ["data/", "run.py", "switch config"],
        "data_bytes": size,
        "artifact_bytes": artifact_size,
        "required_free_bytes": size * 3 + artifact_size * 2 + 1024**3,
        "destination": str(DEV / "switch-operations"),
    }
    s.need(
        shutil.disk_usage(DEV).free > selection["backup"]["required_free_bytes"],
        "Insufficient backup space",
    )
    selection["run_hash"] = s.digest(DEV / "run.py")
    return selection


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


def health(proc, selection):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        s.need(proc.poll() is None, "New server exited; inspect protected server log")
        try:
            response = s.get("/api/health")
            s.need(response.get("success") is True, "Health endpoint refused")
            info = s.get("/api/info")["data"].get("build_info", {})
            s.need(s.owns_port(proc.pid), "New process does not own configured port")
            s.need(
                s.digest(f"/proc/{proc.pid}/exe")
                == selection["binary_hashes"][selection["binary"]],
                "Launched binary hash mismatch",
            )
            s.need(
                s.identity(proc.pid)["cwd"] == selection["source"],
                "Launched source mismatch",
            )
            s.need(
                frontend_hash(P(selection["frontend"]))[0]
                == selection["frontend_tree_sha256"],
                "Frontend changed during launch",
            )
            s.need(
                s.digest(selection["agent_host"])
                == selection["binary_hashes"][selection["agent_host"]],
                "Agent host changed during launch",
            )
            s.need(
                hashlib.sha256(s.get_bytes("/index.html")).hexdigest()
                == s.digest(P(selection["frontend"]) / "index.html"),
                "Served frontend entry differs from selected artifact",
            )
            if selection["recorded_build_sha"]:
                s.need(
                    info.get("revision") == selection["recorded_build_sha"],
                    "Runtime build differs from manifest",
                )
            return info
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
        s.need(
            request.get("configuration", CONFIG) == CONFIG,
            "Instance configuration changed after admission",
        )
        # The MCP admission process must have exited before checking descendants.
        time.sleep(0.2)
        selected = preflight(a)
        s.save(op / "selection.json", selected)
        old = (DEV / "run.py").read_text()
        replacement = new_run(old, selected)
        want = selected["current_process"]
        env = {
            k.decode(): v.decode()
            for k, v in (
                x.split(b"=", 1)
                for x in P(f"/proc/{want['pid']}/environ").read_bytes().split(b"\0")
                if b"=" in x
            )
        }
        # Preserve selected executables outside mutable Cargo output. Frontend absolute
        # paths remain unchanged and are protected by the existing retention checks.
        (op / "artifacts").mkdir(mode=0o700)
        for key in ["binary", "agent_host"]:
            target = (
                op
                / "artifacts"
                / ("server" if key == "binary" else "agent-process-host")
            )
            shutil.copy2(selected[key], target)
            s.need(
                s.digest(target) == selected["binary_hashes"][selected[key]],
                "Candidate changed while preserving",
            )
            selected["binary_hashes"][str(target)] = selected["binary_hashes"][
                selected[key]
            ]
            selected[key] = str(target)
            old_target = op / "artifacts" / ("old-" + target.name)
            shutil.copy2(selected["old_paths"][key], old_target)
            s.need(
                s.digest(old_target) == selected["old_hashes"][key],
                "Old executable changed while preserving",
            )
        for name, front, expected in [
            ("frontend", P(selected["frontend"]), selected["frontend_tree_sha256"]),
            (
                "old-frontend",
                P(selected["old_paths"]["frontend"]),
                selected["old_frontend_hash"],
            ),
        ]:
            shutil.copytree(front, op / "artifacts" / name)
            s.need(
                frontend_hash(op / "artifacts" / name)[0] == expected,
                "Frontend changed while preserving",
            )
        replacement = new_run(old, selected)
        s.save(op / "launch-selection.json", selected)
        stage("stopping")
        s.need(
            s.digest(DEV / "run.py") == selected["run_hash"],
            "Launch configuration changed",
        )
        s.resources()
        s.idle(DEV / "data/db.v2.sqlite")
        s.stop(want, selected["old_hashes"]["binary"])
        stage("backup")
        s.stopped()
        s.idle(DEV / "data/db.v2.sqlite")
        (op / "settings").mkdir(mode=0o700)
        shutil.copy2(DEV / "run.py", op / "settings/run.py")
        shutil.copy2(config_path, op / "settings/switch.json")
        for name in ["start.sh", "start_binary.py", "start_support.py"]:
            shutil.copy2(P(__file__).resolve().parent / name, op / "settings" / name)
        manifest = s.backup_data(DEV / "data", op / "data")
        s.verify_tree(op / "data", manifest)
        s.save(op / "data-manifest.json", manifest)
        for f in op.rglob("*"):
            if f.is_file():
                with f.open("rb") as stream:
                    os.fsync(stream.fileno())
        for d in sorted(
            [op] + [x for x in op.rglob("*") if x.is_dir()],
            key=lambda x: len(x.parts),
            reverse=True,
        ):
            fd = os.open(d, os.O_DIRECTORY)
            try:
                os.fsync(fd)
            finally:
                os.close(fd)
        s.save(
            op / "backup-complete.json",
            {"manifest_sha256": s.digest(op / "data-manifest.json")},
        )
        s.need(
            frontend_hash(P(selected["old_paths"]["frontend"]))[0]
            == selected["old_frontend_hash"],
            "Old frontend changed",
        )
        stage("switching")
        s.stopped()
        again = inspect(a)
        original = s.readjson(op / "selection.json")
        s.need(
            all(again[k] == original[k] for k in again),
            "Candidate changed after backup",
        )
        s.need(
            s.digest(DEV / "run.py") == selected["run_hash"],
            "Launch settings changed after backup",
        )
        s.atomic(DEV / "run.py", replacement)
        stage("starting")
        with (op / "server.log").open("xb") as log:
            proc = subprocess.Popen(
                ["/usr/bin/python3", str(DEV / "run.py")],
                cwd=DEV,
                env=env,
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
        state.update(
            status="succeeded",
            stage="complete",
            process=s.identity(proc.pid),
            build_info=info,
            checks={
                "health": True,
                "artifact_identity": True,
                "frontend_entry": True,
                "db_integrity": True,
                "auth0_ui_mcp": "not_verified",
            },
        )
    except BaseException as error:
        # Raw exceptions/child output can contain secrets; expose only our safe refusals.
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
            "config",
            "asynchronous",
            "status",
            "dry_run",
            "request_id",
            "owner",
            "worker",
            "lock_fd",
        ]
    }
    request = {"owner": a.owner, "arguments": args, "configuration": CONFIG}
    root = DEV / "switch-operations"
    root.mkdir(mode=0o700, exist_ok=True)
    op = root / str(uuid.UUID(a.request_id))
    if (op / "request.json").exists():
        s.need(
            s.readjson(op / "request.json") == request,
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
                s.readjson(op / "request.json") == request,
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
                    prior["status"] in TERMINAL,
                    "Unsettled previous operation; inspect before operator recovery",
                )
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
            with (op / "worker.log").open("xb") as log:
                worker = subprocess.Popen(
                    [
                        sys.executable,
                        str(P(__file__).resolve()),
                        "--config",
                        str(config_path),
                        "--worker",
                        op.name,
                        "--lock-fd",
                        str(lock.fileno()),
                    ],
                    stdin=subprocess.DEVNULL,
                    stdout=log,
                    stderr=log,
                    start_new_session=True,
                    pass_fds=(lock.fileno(),),
                    env={"PATH": "/usr/local/bin:/usr/bin:/bin", "LANG": "C.UTF-8"},
                )
            state["worker_pid"] = worker.pid
            # Worker owns subsequent state writes. Do not overwrite its progress here.
            return state
        return execute(op, request, config_path)


def main():
    p = argparse.ArgumentParser(
        description="Switch externally tested dev artifacts; migrations belong to the server. No automatic rollback."
    )
    p.add_argument("--config", required=True)
    p.add_argument("--binary")
    p.add_argument("--source")
    p.add_argument("--agent-host")
    p.add_argument("--frontend")
    p.add_argument(
        "--frontend-mode", choices=["external", "embedded"], default="external"
    )
    p.add_argument("--manifest")
    p.add_argument("--sha")
    p.add_argument("--retain-worktree", action="store_true")
    p.add_argument("--no-new-work", action="store_true")
    p.add_argument("--request-id")
    p.add_argument("--owner", default="operator")
    p.add_argument("--async", dest="asynchronous", action="store_true")
    p.add_argument("--status", action="store_true")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--worker", help=argparse.SUPPRESS)
    p.add_argument("--lock-fd", type=int, help=argparse.SUPPRESS)
    a = p.parse_args()
    os.umask(0o077)
    config_path = configure(a.config)
    if a.worker:
        # Private worker entry requires an inherited lock, not an admission bypass.
        s.need(
            a.lock_fd is not None
            and os.path.samefile(f"/proc/self/fd/{a.lock_fd}", DEV / "start.lock"),
            "Missing inherited switch lock",
        )
        s.fcntl.flock(a.lock_fd, s.fcntl.LOCK_EX | s.fcntl.LOCK_NB)
        op = DEV / "switch-operations" / str(uuid.UUID(a.worker))
        s.save(op / "worker.json", s.identity(os.getpid()))
        result = execute(op, s.readjson(op / "request.json"), config_path)
    elif a.status:
        result = status(a.request_id, a.owner)
    elif a.dry_run:
        s.need(a.binary, "--binary required")
        result = preflight(a)
    else:
        s.need(a.binary and a.request_id, "--binary and --request-id required")
        result = submit(a, config_path)
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
