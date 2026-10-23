# Reused process/idle/backup primitives from the reviewed dev controller.
import fcntl, hashlib, json, os, pathlib, select, shutil, signal, sqlite3, time, urllib.request, uuid
from contextlib import closing

P = pathlib.Path
DEV = P(__file__).resolve().parent
PORT = 4020


def configure(instance, port):
    global DEV, PORT
    DEV = P(instance)
    PORT = port


TERMINAL = ("succeeded", "failed", "cancelled", "crashed", "audit_failed")


class Refuse(Exception):
    pass


def need(ok, why):
    if not ok:
        raise Refuse(why)


def digest(p):
    with P(p).open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def readjson(p):
    return json.loads(P(p).read_text())


def save(p, j):
    with P(p).open("x") as f:
        json.dump(j, f, indent=2)
        f.flush()
        os.fsync(f.fileno())


def atomic(p, data):
    tmp = p.with_name(p.name + ".switch-" + uuid.uuid4().hex)
    with tmp.open("xb") as f:
        if p.exists():
            os.fchmod(f.fileno(), p.stat().st_mode & 0o777)
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, p)
    fd = os.open(p.parent, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def identity(pid):
    p = P("/proc") / str(pid)
    s = (p / "stat").read_text().rsplit(")", 1)[1].split()
    return {
        "pid": pid,
        "start": s[19],
        "boot": P("/proc/sys/kernel/random/boot_id").read_text().strip(),
        "exe": os.readlink(p / "exe"),
        "cwd": os.readlink(p / "cwd"),
    }


def same(a, b):
    return all(a.get(k) == b.get(k) for k in ["pid", "start", "boot", "exe", "cwd"])


def descendants(pid):
    rows = {}
    for p in P("/proc").iterdir():
        if not p.name.isdigit():
            continue
        try:
            rows[int(p.name)] = int(
                (p / "stat").read_text().rsplit(")", 1)[1].split()[1]
            )
        except FileNotFoundError:
            pass
    found = {pid}
    while True:
        n = found | {p for p, parent in rows.items() if parent in found}
        if n == found:
            return found - {pid}
        found = n


def listeners():
    rows = []
    for family in ["tcp", "tcp6"]:
        for line in P("/proc/net/" + family).read_text().splitlines()[1:]:
            f = line.split()
            if f[3] == "0A" and int(f[1].split(":")[1], 16) == PORT:
                rows.append((f[1], f[9]))
    return rows


def owns_port(pid):
    socks = set()
    for fd in (P("/proc") / str(pid) / "fd").iterdir():
        try:
            socks.add(os.readlink(fd))
        except FileNotFoundError:
            pass
    rows = listeners()
    return (
        len(rows) == 1
        and rows[0][0] == f"0100007F:{PORT:04X}"
        and "socket:[" + rows[0][1] + "]" in socks
    )


class NR(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def get_bytes(path):
    token = (DEV / "data/access-internal.token").read_text().strip()
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORT}" + path, headers={"X-LVK-Internal-Token": token}
    )
    with urllib.request.build_opener(NR).open(req, timeout=5) as r:
        content = r.read(16 * 1024 * 1024 + 1)
        need(len(content) <= 16 * 1024 * 1024, "Instance response exceeds limit")
        return content


def get(path):
    return json.loads(get_bytes(path))


def resources():
    j = get("/api/resource-coordination/snapshot")
    need(j.get("success"), "Resource snapshot failed")
    need(
        not any(j["data"][k] for k in ["resources", "holders", "queue"]),
        "Resources registered/busy: coordinate explicitly; do not bypass",
    )


def ro(path):
    c = sqlite3.connect(P(path).resolve().as_uri() + "?mode=ro", uri=True, timeout=3)
    c.execute("PRAGMA query_only=ON")
    return closing(c)


def idle(path):
    with ro(path) as c:
        c.execute("BEGIN")
        rules = {
            "agent_runs": ("status", TERMINAL),
            "agent_run_attempts": ("status", TERMINAL),
            "execution_processes": ("status", ("completed", "failed", "killed")),
            "workflow_runs": ("status", ("succeeded", "failed", "canceled")),
            "workflow_attempts": (
                "status",
                ("succeeded", "failed", "cancelled", "canceled"),
            ),
            "resource_operations": ("status", ("succeeded", "cancelled", "recovered")),
            "agent_run_commands": ("delivery_status", ("delivered", "failed")),
            "orchestration_outbox": ("delivery_status", ("delivered", "failed")),
            "bridge_commands": ("status", ("done",)),
        }
        for t, (col, ends) in rules.items():
            n = c.execute(
                f"SELECT count(*) FROM {t} WHERE {col} IS NULL OR {col} NOT IN ("
                + ",".join("?" for _ in ends)
                + ")",
                ends,
            ).fetchone()[0]
            need(n == 0, f"Unfinished work: {t} count={n}")
        need(
            c.execute(
                "SELECT count(*) FROM scheduled_tasks WHERE enabled!=0"
            ).fetchone()[0]
            == 0,
            "Enabled schedule exists",
        )
        need(
            c.execute(
                "SELECT count(*) FROM agent_process_registry WHERE registry_status != 'exited'"
            ).fetchone()[0]
            == 0,
            "Unsettled process registry",
        )
        need(
            c.execute(
                "SELECT count(*) FROM integration_runs WHERE status NOT IN ('succeeded','failed','cancelled')"
            ).fetchone()[0]
            == 0,
            "Unsettled Integration",
        )
        need(
            c.execute("SELECT count(*) FROM integration_reservations").fetchone()[0]
            == 0,
            "Integration reservations exist",
        )


def process_files(p):
    # exec/exit can briefly revoke procfs access even for the same UID. Retry a
    # bounded observation; never silently ignore a persistently unreadable process.
    for attempt in range(20):
        try:
            if (p / "stat").read_text().rsplit(")", 1)[1].split()[0] == "Z":
                return {}, []
            env = dict(
                x.split(b"=", 1)
                for x in (p / "environ").read_bytes().split(b"\0")
                if b"=" in x
            )
            targets = []
            for fd in (p / "fd").iterdir():
                try:
                    targets.append(os.readlink(fd))
                except FileNotFoundError:
                    pass
            return env, targets
        except FileNotFoundError:
            return {}, []
        except (PermissionError, ProcessLookupError):
            time.sleep(0.01)
    raise Refuse("Cannot inspect live process")


def stopped():
    need(not listeners(), "Instance port still in use")
    for p in P("/proc").iterdir():
        if not p.name.isdigit() or int(p.name) == os.getpid():
            continue
        env, targets = process_files(p)
        need(
            env.get(b"LVK_INSTANCE_DIR") != str(DEV).encode(),
            f"Instance process remains: PID {p.name}",
        )
        need(
            not any(t.startswith(str(DEV / "data") + "/") for t in targets),
            f"Instance data open by PID {p.name}",
        )
    # The caller's no-new-work coordination excludes actors opening new FDs later.


def pre_stop_writers(allowed_pid):
    for p in P("/proc").iterdir():
        if not p.name.isdigit() or int(p.name) in [os.getpid(), allowed_pid]:
            continue
        _, targets = process_files(p)
        need(
            not any(t.startswith(str(DEV / "data") + "/") for t in targets),
            f"Other instance data reader/writer: PID {p.name}",
        )


def stop(want, expected_hash, require_port=True):
    pid = want["pid"]
    fd = os.pidfd_open(pid)
    try:
        need(same(identity(pid), want), "PID identity changed")
        need(
            digest(f"/proc/{pid}/exe") == expected_hash, "Live executable hash changed"
        )
        need(not descendants(pid), "Dev has child processes; finish them first")
        pre_stop_writers(pid)
        if require_port:
            need(owns_port(pid), "Instance port does not belong to expected dev")
        signal.pidfd_send_signal(fd, signal.SIGTERM)
        need(
            bool(select.select([fd], [], [], 60)[0]),
            "TERM timeout; no SIGKILL, no automatic recovery",
        )
    finally:
        os.close(fd)
    # SQLx/process shutdown may take a moment to close all descriptors.
    stopped()


def tree_manifest(root):
    result = {}
    for p in sorted(root.rglob("*")):
        name = str(p.relative_to(root))
        if p.is_symlink():
            result[name] = {"link": os.readlink(p)}
        elif p.is_file():
            result[name] = {"sha256": digest(p), "bytes": p.stat().st_size}
    return result


def verify_tree(root, manifest):
    need(tree_manifest(root) == manifest, "Backup data manifest mismatch")


def backup_data(source, dest):
    need(not dest.exists(), "Backup destination already exists")
    shutil.copytree(
        source,
        dest,
        symlinks=True,
        ignore=lambda d, n: [
            x
            for x in n
            if P(d) == source
            and x
            in (
                "db.v2.sqlite",
                "db.v2.sqlite-wal",
                "db.v2.sqlite-shm",
                "db.v2.sqlite-journal",
            )
        ],
    )
    with ro(source / "db.v2.sqlite") as src, closing(
        sqlite3.connect(dest / "db.v2.sqlite")
    ) as dst:
        src.backup(dst)
        need(
            dst.execute("PRAGMA journal_mode=DELETE").fetchone()[0] == "delete",
            "Backup must be standalone, without WAL",
        )
    with ro(dest / "db.v2.sqlite") as c:
        need(
            c.execute("PRAGMA integrity_check").fetchall() == [("ok",)],
            "Backup integrity failed",
        )
        need(
            not c.execute("PRAGMA foreign_key_check").fetchall(),
            "Backup FK check failed",
        )
    return tree_manifest(dest)
