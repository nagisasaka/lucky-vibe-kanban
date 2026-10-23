"""Linux isolation/fault tests. All processes, ports and DBs are test-owned."""

import argparse
import contextlib
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import unittest
import uuid
from unittest.mock import patch

import start_binary as engine
import start_support as support

HERE = Path(__file__).resolve().parent


class SwitchTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lvk-switch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.instance = self.root / "instance"
        self.instance.mkdir()
        (self.instance / "data").mkdir()
        (self.instance / "data/config.json").write_text("{}")
        (self.instance / "data/access-internal.token").write_text(
            "fixture-private-token"
        )
        (self.instance / "data/auth-secret").write_text("fixture-auth-secret")
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            self.port = sock.getsockname()[1]
        self.config = self.instance / "switch.json"
        self.config.write_text(
            json.dumps({"instance": str(self.instance), "port": self.port})
        )
        self.db = self.instance / "data/db.v2.sqlite"
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            for table, column in [
                ("agent_runs", "status"),
                ("agent_run_attempts", "status"),
                ("execution_processes", "status"),
                ("workflow_runs", "status"),
                ("workflow_attempts", "status"),
                ("resource_operations", "status"),
                ("agent_run_commands", "delivery_status"),
                ("orchestration_outbox", "delivery_status"),
                ("bridge_commands", "status"),
                ("agent_process_registry", "registry_status"),
                ("integration_runs", "status"),
            ]:
                db.execute(f"CREATE TABLE {table}({column} TEXT)")
            db.executescript(
                'CREATE TABLE scheduled_tasks(enabled INTEGER); CREATE TABLE integration_reservations(id TEXT); CREATE TABLE workspaces(container_ref TEXT); CREATE TABLE user_data(value TEXT); INSERT INTO user_data VALUES("committed");'
            )
        self.builds = []
        for rev in [1, 2]:
            source = self.instance / "builds" / str(rev)
            front = source / "packages/local-web/dist"
            front.mkdir(parents=True)
            (front / "index.html").write_text(f"frontend-{rev}")
            binary = source / "server"
            host = source / "agent-process-host"
            subprocess.run(
                [
                    "cc",
                    "-Wall",
                    "-Wextra",
                    f"-DREVISION={rev}",
                    f'-DFRONTEND="{front}"',
                    str(HERE / "fixture_server.c"),
                    "-l:libsqlite3.so.0",
                    "-o",
                    str(binary),
                ],
                check=True,
                capture_output=True,
            )
            host.write_bytes(binary.read_bytes())
            host.chmod(0o700)
            self.builds.append(
                dict(
                    binary=str(binary),
                    agent_host=str(host),
                    source=str(source),
                    frontend=str(front),
                )
            )
        self.write_run(self.builds[0])
        self.old = subprocess.Popen(
            [sys.executable, str(self.instance / "run.py")],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.wait_port()
        engine.configure(self.config)

    def tearDown(self):
        # Do not signal raw PIDs from untrusted files. Record pidfds while processes
        # are known test children, or verify instance+exe before opening their handle.
        for op in (self.instance / "switch-operations").glob("*/result.json"):
            result = json.loads(op.read_text())
            if result["status"] not in engine.TERMINAL:
                self.fail("test left an active switch worker")
        for p in Path("/proc").iterdir():
            if not p.name.isdigit():
                continue
            try:
                exe = os.readlink(p / "exe")
                if exe.startswith(str(self.instance) + "/"):
                    fd = os.pidfd_open(int(p.name))
                    try:
                        if os.readlink(p / "exe") == exe:
                            signal.pidfd_send_signal(fd, signal.SIGTERM)
                    finally:
                        os.close(fd)
            except (ProcessLookupError, FileNotFoundError):
                pass
        self.old.wait(timeout=5)
        # Wait for all test-owned sockets to disappear before deleting fixture data.
        deadline = time.monotonic() + 5
        while support.listeners() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertFalse(support.listeners())
        self.temp.cleanup()

    def write_run(self, selection, extra=None):
        env = {
            "LVK_INSTANCE_DIR": str(self.instance),
            "BACKEND_PORT": str(self.port),
            "HOST": "127.0.0.1",
            "DISABLE_WORKTREE_CLEANUP": "1",
            "VIBE_KANBAN_AGENT_PROCESS_HOST": selection["agent_host"],
            "FIXTURE_SECRET": "do-not-print-this",
        }
        env.update(extra or {})
        (self.instance / "run.py").write_text(f"""import os
from pathlib import Path
root=Path(__file__).resolve().parent
source=Path({selection['source']!r})
lvk_frontend_dependency={selection['frontend']!r}
env=os.environ.copy()
env.update({env!r})
os.chdir(source)
(root/'server.pid').write_text(str(os.getpid()))
os.execve({selection['binary']!r},[{selection['binary']!r}],env)
""")

    def wait_port(self):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            with socket.socket() as sock:
                if sock.connect_ex(("127.0.0.1", self.port)) == 0:
                    return
            self.assertIsNone(self.old.poll())
            time.sleep(0.02)
        self.fail("fixture did not listen")

    def args(self, revision=1, **extra):
        values = dict(
            self.builds[revision],
            manifest=None,
            sha=None,
            retain_worktree=False,
            no_new_work=True,
        )
        values.update(extra)
        return argparse.Namespace(**values)

    def cli(self, revision=1, request=None, extra=()):
        command = [
            sys.executable,
            str(HERE / "start_binary.py"),
            "--config",
            str(self.config),
            "--request-id",
            request or str(uuid.uuid4()),
            "--no-new-work",
        ]
        for k, v in self.builds[revision].items():
            command.extend(["--" + k.replace("_", "-"), v])
        return command + list(extra)

    def run_cli(self, command):
        result = subprocess.run(command, text=True, capture_output=True, timeout=20)
        self.assertNotIn("do-not-print-this", result.stdout + result.stderr)
        self.assertNotIn("fixture-private-token", result.stdout + result.stderr)
        return json.loads(result.stdout)

    def settle(self, request):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            state = engine.status(request, "operator")
            if state["status"] in engine.TERMINAL:
                return state
            time.sleep(0.05)
        self.fail("worker did not settle")

    def test_two_artifacts_and_server_owned_migration(self):
        for revision in [0, 1]:
            state = self.run_cli(self.cli(revision))
            self.assertEqual(state["status"], "succeeded", state)
            op = Path(state["result_path"]).parent
            self.assertEqual(
                (op / "data/auth-secret").read_text(), "fixture-auth-secret"
            )
            self.assertEqual(
                (self.instance / "data/auth-secret").read_text(), "fixture-auth-secret"
            )
            self.assertFalse((op / "data/builds").exists())
            self.assertFalse((op / "data/runtime").exists())
            self.assertTrue((op / "artifacts/old-server").is_file())
            self.assertEqual(state["checks"]["auth0_ui_mcp"], "not_verified")
            with contextlib.closing(
                sqlite3.connect(op / "data/db.v2.sqlite")
            ) as db, db:
                self.assertEqual(db.execute("PRAGMA user_version").fetchone()[0], 0)
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            self.assertEqual(db.execute("PRAGMA user_version").fetchone()[0], 2)

    def test_async_duplicate_and_caller_exit(self):
        request = str(uuid.uuid4())
        command = self.cli(request=request, extra=["--async"])
        state = self.run_cli(command)
        self.assertEqual(state["operation_id"], request)
        state = self.settle(request)
        self.assertEqual(state["status"], "succeeded", state)
        repeated = self.run_cli(command)
        self.assertEqual(repeated["operation_id"], request)
        self.assertEqual(repeated["process"], state["process"])
        changed = self.run_cli(self.cli(0, request=request, extra=["--async"]))
        self.assertEqual(changed["status"], "refused")
        with self.assertRaises(support.Refuse):
            engine.status(request, "someone-else")

    def test_requester_sigkill_does_not_kill_transaction(self):
        request = str(uuid.uuid4())
        command = self.cli(request=request, extra=["--async"])
        receipt = self.root / "admission.json"
        script = (
            "import subprocess,pathlib,time; r=subprocess.run("
            + repr(command)
            + ",capture_output=True,check=True); pathlib.Path("
            + repr(str(receipt))
            + ").write_bytes(r.stdout); time.sleep(30)"
        )
        caller = subprocess.Popen([sys.executable, "-c", script])
        try:
            deadline = time.monotonic() + 5
            while not receipt.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(receipt.exists())
            caller.kill()
            caller.wait(timeout=5)
            state = self.settle(request)
            self.assertEqual(state["status"], "succeeded", state)
            self.assertEqual(
                engine.status(request, "operator")["operation_id"], request
            )
        finally:
            if caller.poll() is None:
                caller.kill()
                caller.wait()

    def test_worker_sigkill_is_durable_interrupted_without_replay(self):
        request = str(uuid.uuid4())
        state = self.run_cli(self.cli(request=request, extra=["--async"]))
        fd = os.pidfd_open(state["worker_pid"])
        try:
            signal.pidfd_send_signal(fd, signal.SIGKILL)
        finally:
            os.close(fd)
        result = self.settle(request)
        self.assertEqual(result["status"], "interrupted")
        repeated = self.run_cli(self.cli(request=request, extra=["--async"]))
        self.assertEqual(repeated["status"], "interrupted")
        self.assertIsNone(self.old.poll())

    def test_registered_resources_and_changed_identity_refused(self):
        with patch.object(
            support,
            "get",
            return_value={
                "success": True,
                "data": {"resources": [{}], "holders": [], "queue": []},
            },
        ):
            with self.assertRaises(support.Refuse):
                engine.preflight(self.args())
        want = engine.observation()
        want["start"] = "wrong"
        with self.assertRaises(support.Refuse):
            support.stop(want, support.digest(self.builds[0]["binary"]))
        self.assertIsNone(self.old.poll())

    def test_incomplete_admission_is_not_replayed(self):
        request = str(uuid.uuid4())
        op = self.instance / "switch-operations" / request
        op.mkdir(parents=True)
        # Simulate a crash after publishing request but before writing result. Older
        # or manually damaged records are treated as interrupted, never restarted.
        command = self.cli(request=request)
        args = vars(self.args())
        args["frontend_mode"] = "external"
        support.save(op / "request.json", {"owner": "operator", "arguments": args})
        result = engine.status(request, "operator")
        self.assertEqual(result["status"], "interrupted")
        self.assertIsNone(self.old.poll())

    def test_embedded_manifest_and_corruption_refusal(self):
        selection = self.builds[1]
        tree, files = engine.frontend_hash(Path(selection["frontend"]))
        manifest = self.root / "manifest.json"
        content = {
            "artifacts": [
                {"path": selection[k], "sha256": support.digest(selection[k])}
                for k in ["binary", "agent_host"]
            ],
            "frontend": {
                "path": selection["frontend"],
                "tree_sha256": tree,
                "files": files,
            },
        }
        manifest.write_text(json.dumps(content))
        selected = engine.inspect(
            self.args(frontend_mode="embedded", manifest=str(manifest))
        )
        self.assertEqual(selected["frontend_mode"], "embedded")
        content["artifacts"][0]["sha256"] = "wrong"
        manifest.write_text(json.dumps(content))
        with self.assertRaises(support.Refuse):
            engine.inspect(self.args(frontend_mode="embedded", manifest=str(manifest)))
        self.assertIsNone(self.old.poll())

    def test_corrupt_old_copy_refused_before_stop(self):
        op = self.instance / "copy-failure"
        op.mkdir()
        support.save(op / "result.json", {"status": "accepted"})
        original = engine.shutil.copy2

        def corrupt(source, destination, *args, **kwargs):
            result = original(source, destination, *args, **kwargs)
            if Path(destination).name == "old-agent-process-host":
                Path(destination).write_bytes(b"corrupt")
            return result

        with patch.object(engine.shutil, "copy2", side_effect=corrupt):
            result = engine.execute(
                op, {"owner": "operator", "arguments": vars(self.args())}, self.config
            )
        self.assertEqual(result["status"], "failed")
        self.assertIsNone(self.old.poll())

    def test_mcp_driver_termination_cleans_owned_server(self):
        directory = self.root / "driver"
        directory.mkdir()
        driver = subprocess.Popen(
            [sys.executable, str(HERE / "mcp_fixture.py"), str(directory)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            deadline = time.monotonic() + 10
            while (
                not (directory / "fixture.json").exists()
                and time.monotonic() < deadline
            ):
                time.sleep(0.02)
            metadata = json.loads((directory / "fixture.json").read_text())
            instance = Path(metadata["instance"])
            driver.terminate()
            _, error = driver.communicate(timeout=20)
            self.assertEqual(driver.returncode, 0, error)
            self.assertFalse(instance.exists())
            for p in Path("/proc").iterdir():
                if not p.name.isdigit():
                    continue
                try:
                    self.assertFalse(
                        os.readlink(p / "exe").startswith(str(instance) + "/")
                    )
                except FileNotFoundError:
                    pass
        finally:
            if driver.poll() is None:
                driver.terminate()
                driver.wait(timeout=180)

    def test_pid_mismatch_does_not_stop_unrelated(self):
        innocent = subprocess.Popen(["sleep", "30"])
        try:
            (self.instance / "server.pid").write_text(str(innocent.pid))
            result = self.run_cli(self.cli())
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["stage"], "preflight")
            self.assertIsNone(innocent.poll())
            self.assertIsNone(self.old.poll())
        finally:
            innocent.terminate()
            innocent.wait()

    def test_busy_and_missing_artifact(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            db.execute("INSERT INTO agent_runs VALUES('running')")
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertIsNone(self.old.poll())
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            db.execute("DELETE FROM agent_runs")
        Path(self.builds[1]["binary"]).unlink()
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertIsNone(self.old.poll())

    def test_backup_failure_preserves_data_and_old_reference(self):
        args = self.args()
        op = self.instance / "failure"
        op.mkdir()
        request = {"owner": "operator", "arguments": vars(args)}
        support.save(op / "result.json", {"status": "accepted"})
        before = (self.instance / "run.py").read_bytes()
        with patch.object(
            support, "backup_data", side_effect=OSError("private-secret")
        ):
            result = engine.execute(op, request, self.config)
        self.assertEqual(result["stage"], "backup")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["reason"], "OSError")
        self.assertEqual((self.instance / "run.py").read_bytes(), before)
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            self.assertEqual(
                db.execute("SELECT value FROM user_data").fetchone()[0], "committed"
            )

    def test_health_failure_no_rollback(self):
        self.write_run(self.builds[0], {"FIXTURE_BAD_HEALTH": "1"})
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["stage"], "health", result)
        self.assertTrue(
            (Path(result["result_path"]).parent / "backup-complete.json").exists()
        )
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            self.assertEqual(db.execute("PRAGMA user_version").fetchone()[0], 2)

    def test_start_failure_no_rollback(self):
        self.write_run(self.builds[0], {"FIXTURE_EXIT": "1"})
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["stage"], "health", result)
        self.assertTrue(
            (Path(result["result_path"]).parent / "data/db.v2.sqlite").is_file()
        )

    def test_lock_refuses_conflict(self):
        with (self.instance / "start.lock").open("a") as lock:
            support.fcntl.flock(lock, support.fcntl.LOCK_EX)
            result = self.run_cli(self.cli())
            self.assertEqual(result["status"], "refused")
            self.assertIsNone(self.old.poll())

    def test_wal_backup_contains_committed_rows(self):
        # WAL remains open in a separate DB; this primitive test does not cut over it.
        source = self.root / "wal"
        source.mkdir()
        db = sqlite3.connect(source / "db.v2.sqlite")
        try:
            db.execute("PRAGMA journal_mode=WAL")
            db.execute("CREATE TABLE t(value TEXT)")
            db.execute("INSERT INTO t VALUES('wal-committed')")
            db.commit()
            dest = self.root / "backup"
            support.backup_data(source, dest)
            with contextlib.closing(
                sqlite3.connect(dest / "db.v2.sqlite")
            ) as copied, copied:
                self.assertEqual(
                    copied.execute("SELECT value FROM t").fetchone()[0], "wal-committed"
                )
        finally:
            db.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
