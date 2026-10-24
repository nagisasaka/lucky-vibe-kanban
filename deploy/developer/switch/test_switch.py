"""Linux isolation/fault tests. All processes, ports and DBs are test-owned."""

import argparse
import contextlib
import json
import hashlib
import shutil
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
        self.config = self.instance / "launch.json"
        self.config.write_text(json.dumps({"environment": {}, "port": self.port}))
        self.config.chmod(0o600)
        shutil.copytree(
            HERE, self.instance / "switch", ignore=shutil.ignore_patterns("__pycache__")
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
            bundle = self.root / f"build-{rev}"
            bundle.mkdir()
            html = f"frontend-{rev}"
            assets = {
                "frontend/index.html": hashlib.sha256(html.encode()).hexdigest(),
                "sounds/fixture": "fixture",
                "scripts/fixture": "fixture",
            }
            payload = json.dumps({"embedded": True, "assets": assets})
            subprocess.run(
                [
                    "cc",
                    "-Wall",
                    "-Wextra",
                    f"-DREVISION={rev}",
                    "-DHTML=" + json.dumps(html),
                    "-DPAYLOAD=" + json.dumps(payload),
                    str(HERE / "fixture_server.c"),
                    "-l:libsqlite3.so.0",
                    "-o",
                    str(bundle / "server"),
                ],
                check=True,
                capture_output=True,
            )
            for name in ["agent-process-host", "vibe-kanban-mcp"]:
                shutil.copy2(bundle / "server", bundle / name)
            manifest = {
                "format": 1,
                "assets": assets,
                "files": {
                    name: support.digest(bundle / name) for name in engine.PAYLOAD
                },
            }
            (bundle / "bundle.json").write_text(json.dumps(manifest))
            self.builds.append({"bundle": str(bundle)})
        (self.instance / "releases").mkdir()
        self.write_run(self.builds[0])
        self.old = subprocess.Popen(
            [
                sys.executable,
                str(self.instance / "switch/launcher.py"),
                str(self.instance),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.wait_port()
        engine.configure(self.instance)

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
        bundle = Path(selection["bundle"])
        release = self.instance / "releases/initial"
        if not release.exists():
            shutil.copytree(bundle, release)
        manifest = json.loads((bundle / "bundle.json").read_text())
        (self.instance / "current.json").write_text(
            json.dumps({"release": str(release), "manifest": manifest})
        )
        env = {
            "PATH": "/usr/local/bin:/usr/bin:/bin",
            "FIXTURE_SECRET": "do-not-print-this",
        }
        env.update(extra or {})
        self.config.write_text(json.dumps({"port": self.port, "environment": env}))
        self.config.chmod(0o600)

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
        values = dict(self.builds[revision], no_new_work=True)
        values.update(extra)
        return argparse.Namespace(**values)

    def cli(self, revision=1, request=None, extra=()):
        command = [
            sys.executable,
            str(HERE / "start_binary.py"),
            "--instance",
            str(self.instance),
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

    def test_copied_bundle_survives_hidden_build_sources(self):
        original = engine.preserve
        for revision in [0, 1]:
            request = str(uuid.uuid4())
            op = self.instance / "switch-operations" / request
            op.mkdir(parents=True)
            args = self.args(revision)
            support.save(op / "result.json", {"status": "accepted"})
            request_data = {"arguments": vars(args), "snapshot": engine.snapshot()}
            support.save(op / "request.json", request_data)

            def hidden(selected, operation):
                copied = original(selected, operation)
                Path(selected["bundle"]).rename(Path(selected["bundle"] + "-hidden"))
                return copied

            with patch.object(engine, "preserve", side_effect=hidden):
                result = engine.execute(op, request_data, self.config)
            self.assertEqual(result["status"], "succeeded", result)
            self.assertEqual(
                support.get_bytes("/index.html"), f"frontend-{revision + 1}".encode()
            )
        self.assertEqual(len(list((self.instance / "releases").iterdir())), 2)
        self.assertEqual(
            len(list((self.instance / "switch-operations").glob("*/data"))), 1
        )

    def test_corrupt_copy_refused_and_failed_request_fences_growth(self):
        original = engine.shutil.copy2

        def corrupt(source, dest, *args, **kwargs):
            result = original(source, dest, *args, **kwargs)
            if Path(dest).name == "agent-process-host":
                Path(dest).write_bytes(b"corrupt")
            return result

        op = self.instance / "switch-operations" / str(uuid.uuid4())
        op.mkdir(parents=True)
        request = {"arguments": vars(self.args()), "snapshot": engine.snapshot()}
        support.save(op / "result.json", {"status": "accepted"})
        with patch.object(engine.shutil, "copy2", side_effect=corrupt):
            result = engine.execute(op, request, self.config)
        self.assertEqual(result["status"], "failed", result)
        self.assertIsNone(self.old.poll())
        count = len(list((self.instance / "releases").iterdir()))
        refused = self.run_cli(self.cli())
        self.assertEqual(refused["status"], "refused", refused)
        self.assertEqual(len(list((self.instance / "releases").iterdir())), count)

    def test_manifest_claim_cannot_prove_embedded_payload(self):
        bundle = Path(self.builds[1]["bundle"])
        m = json.loads((bundle / "bundle.json").read_text())
        m["assets"]["frontend/index.html"] = "untrue"
        (bundle / "bundle.json").write_text(json.dumps(m))
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed", result)
        self.assertIsNone(self.old.poll())

    def test_snapshot_mismatch_is_pre_stop_failure(self):
        request = str(uuid.uuid4())
        snapshot = engine.snapshot()
        snapshot["identity"]["start"] = "not-self"
        result = subprocess.run(
            self.cli(request=request, extra=["--async", "--snapshot-stdin"]),
            input=json.dumps(snapshot),
            capture_output=True,
            text=True,
            timeout=5,
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        state = self.settle(request)
        self.assertEqual(state["status"], "failed", state)
        self.assertIsNone(self.old.poll())

    def test_previous_backup_survives_failed_next_backup(self):
        first = self.run_cli(self.cli(0))
        self.assertEqual(first["status"], "succeeded", first)
        backup = Path(first["result_path"]).parent / "data"
        manifest = support.tree_manifest(backup)
        op = self.instance / "switch-operations" / str(uuid.uuid4())
        op.mkdir()
        support.save(op / "result.json", {"status": "accepted"})
        request = {"arguments": vars(self.args()), "snapshot": engine.snapshot()}
        with patch.object(support, "backup_data", side_effect=OSError("private")):
            result = engine.execute(op, request, self.config)
        self.assertEqual(result["stage"], "backup", result)
        self.assertEqual(support.tree_manifest(backup), manifest)
        self.assertEqual(len(list((self.instance / "releases").iterdir())), 3)

    def test_installed_cli_resolves_instance_without_duplicate_input(self):
        command = self.cli()
        command[1] = str(self.instance / "switch/start_binary.py")
        index = command.index("--instance")
        del command[index : index + 2]
        result = self.run_cli(command + ["--dry-run"])
        self.assertEqual(result["snapshot"]["instance"], str(self.instance))
        self.assertIsNone(self.old.poll())

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
            self.assertEqual(len(list((self.instance / "releases").iterdir())), 2)
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

    def test_registered_unused_resources_allowed_but_holders_and_queue_refused(self):
        snapshot = {
            "success": True,
            "data": {
                "resources": [{"id": "unrelated-device"}],
                "holders": [],
                "queue": [],
            },
        }
        with patch.object(support, "get", return_value=snapshot):
            engine.preflight(self.args())
            for key in ("holders", "queue"):
                snapshot["data"][key] = [{"operation_id": "active-operation"}]
                with self.assertRaises(support.Refuse):
                    engine.preflight(self.args())
                snapshot["data"][key] = []
        with patch.object(support, "get", side_effect=ConnectionError):
            with self.assertRaises(ConnectionError):
                engine.preflight(self.args())
        want = engine.observation()
        want["start"] = "wrong"
        with self.assertRaises(support.Refuse):
            support.stop(
                want, support.digest(str(self.instance / "releases/initial/server"))
            )
        self.assertIsNone(self.old.poll())

    def test_incomplete_admission_is_not_replayed(self):
        request = str(uuid.uuid4())
        op = self.instance / "switch-operations" / request
        op.mkdir(parents=True)
        # Simulate a crash after publishing request but before writing result. Older
        # or manually damaged records are treated as interrupted, never restarted.
        command = self.cli(request=request)
        args = vars(self.args())

        support.save(op / "request.json", {"owner": "operator", "arguments": args})
        result = engine.status(request, "operator")
        self.assertEqual(result["status"], "interrupted")
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
            self.assertEqual(result["status"], "refused")
            self.assertIsNone(innocent.poll())
            self.assertIsNone(self.old.poll())
        finally:
            innocent.terminate()
            innocent.wait()

    def add_stale_history(self):
        # Deliberately unresolved logical history, with no live process attached.
        # Even stale 'running' text is not proof of a currently executing process.
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            db.executescript("""
                INSERT INTO agent_runs VALUES('audit_failed'),('running');
                INSERT INTO agent_run_attempts VALUES('audit_failed'),('running');
                INSERT INTO agent_process_registry VALUES('reserved');
                INSERT INTO integration_runs VALUES('blocked');
                INSERT INTO integration_reservations VALUES('logical-reservation');
                INSERT INTO agent_run_commands VALUES('pending');
                INSERT INTO orchestration_outbox VALUES('pending');
                INSERT INTO bridge_commands VALUES('pending');
                INSERT INTO scheduled_tasks VALUES(1);
            """)

    def test_stale_history_does_not_gate_cutover_or_get_rewritten(self):
        self.add_stale_history()
        state = self.run_cli(self.cli())
        self.assertEqual(state["status"], "succeeded", state)
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            self.assertEqual(
                db.execute(
                    "SELECT registry_status FROM agent_process_registry"
                ).fetchall(),
                [("reserved",)],
            )
            self.assertEqual(
                db.execute("SELECT status FROM integration_runs").fetchall(),
                [("blocked",)],
            )
            self.assertEqual(
                db.execute("SELECT delivery_status FROM agent_run_commands").fetchall(),
                [("pending",)],
            )

    def test_current_instance_process_blocks_even_with_terminal_history(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            db.execute("INSERT INTO agent_runs VALUES('succeeded')")
        current = subprocess.Popen(
            ["sleep", "30"],
            env={**os.environ, "LVK_INSTANCE_DIR": str(self.instance)},
            start_new_session=True,
        )
        try:
            dry_command = self.cli()
            dry_command.remove("--no-new-work")
            result = self.run_cli(dry_command + ["--dry-run"])
            self.assertEqual(result["status"], "refused", result)
            self.assertIn("Other instance process remains", result["reason"])
            self.assertFalse((self.instance / "switch-operations").exists())
            self.assertFalse((self.instance / "start.lock").exists())
            result = self.run_cli(self.cli())
            self.assertEqual(result["status"], "failed", result)
            self.assertIn("Other instance process remains", result["reason"])
            self.assertIsNone(self.old.poll())
            self.assertIsNone(current.poll())
        finally:
            current.terminate()
            current.wait()

    def test_external_data_user_blocks_with_stale_history(self):
        self.add_stale_history()
        env = {k: v for k, v in os.environ.items() if k != "LVK_INSTANCE_DIR"}
        reader = subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import sys,time; f=open(sys.argv[1],'rb'); print('ready',flush=True); time.sleep(30)",
                str(self.db),
            ],
            env=env,
            stdout=subprocess.PIPE,
            text=True,
        )
        try:
            self.assertEqual(reader.stdout.readline().strip(), "ready")
            result = self.run_cli(self.cli())
            self.assertEqual(result["status"], "failed", result)
            self.assertIn("data reader/writer", result["reason"])
            self.assertIsNone(self.old.poll())
        finally:
            reader.terminate()
            reader.wait()
            reader.stdout.close()

    def test_server_child_without_data_fd_blocks(self):
        self.old.terminate()
        self.old.wait()
        self.write_run(self.builds[0], {"FIXTURE_CHILD": "1"})
        self.old = subprocess.Popen(
            [
                sys.executable,
                str(self.instance / "switch/launcher.py"),
                str(self.instance),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.wait_port()
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed", result)
        self.assertIn("child processes", result["reason"])
        self.assertIsNone(self.old.poll())

    def test_dry_run_without_attestation_and_real_switch_still_requires_it(self):
        self.add_stale_history()
        command = self.cli()
        command.remove("--no-new-work")
        before = (self.instance / "current.json").read_bytes()
        result = self.run_cli(command + ["--dry-run"])
        self.assertIn("backup", result)
        self.assertFalse((self.instance / "switch-operations").exists())
        self.assertFalse((self.instance / "start.lock").exists())
        self.assertEqual((self.instance / "current.json").read_bytes(), before)
        self.assertIsNone(self.old.poll())
        result = self.run_cli(command)
        self.assertEqual(result["status"], "failed")
        self.assertIn("--no-new-work", result["reason"])
        self.assertEqual((self.instance / "current.json").read_bytes(), before)
        self.assertIsNone(self.old.poll())

    def test_unrelated_process_allowed_and_missing_artifact_refused(self):
        innocent = subprocess.Popen(["sleep", "30"])
        try:
            engine.preflight(self.args())
            Path(str(Path(self.builds[1]["bundle"]) / "server")).unlink()
            result = self.run_cli(self.cli())
            self.assertEqual(result["status"], "failed")
            self.assertIsNone(self.old.poll())
            self.assertIsNone(innocent.poll())
        finally:
            innocent.terminate()
            innocent.wait()

    def test_backup_failure_preserves_data_and_old_reference(self):
        args = self.args()
        op = self.instance / "failure"
        op.mkdir()
        request = {
            "owner": "operator",
            "arguments": vars(args),
            "snapshot": engine.snapshot(),
        }
        support.save(op / "result.json", {"status": "accepted"})
        before = (self.instance / "current.json").read_bytes()
        with patch.object(
            support, "backup_data", side_effect=OSError("private-secret")
        ):
            result = engine.execute(op, request, self.config)
        self.assertEqual(result["stage"], "backup")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["reason"], "OSError")
        self.assertEqual((self.instance / "current.json").read_bytes(), before)
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            self.assertEqual(
                db.execute("SELECT value FROM user_data").fetchone()[0], "committed"
            )

    def restart_old(self):
        self.old.terminate()
        self.old.wait(timeout=5)
        self.old = subprocess.Popen(
            [
                sys.executable,
                str(self.instance / "switch/launcher.py"),
                str(self.instance),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.wait_port()

    def test_changed_launch_settings_refused_before_stop(self):
        self.write_run(self.builds[0], {"ADDED_SETTING": "1"})
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "refused", result)
        self.assertIsNone(self.old.poll())

    def test_health_failure_no_rollback(self):
        self.write_run(self.builds[0], {"FIXTURE_BAD_HEALTH": "1"})
        self.restart_old()
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["stage"], "health", result)
        self.assertTrue(
            (Path(result["result_path"]).parent / "backup-complete.json").exists()
        )
        with contextlib.closing(sqlite3.connect(self.db)) as db, db:
            self.assertEqual(db.execute("PRAGMA user_version").fetchone()[0], 2)

    def test_startup_probe_failure_keeps_old_server(self):
        # Candidate exits after startup migration; old server keeps serving.
        bundle = Path(self.builds[1]["bundle"])
        (bundle / "server").write_text("#!/bin/sh\nexit 1\n")
        manifest = json.loads((bundle / "bundle.json").read_text())
        manifest["files"]["server"] = support.digest(bundle / "server")
        (bundle / "bundle.json").write_text(json.dumps(manifest))
        result = self.run_cli(self.cli())
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["stage"], "preflight", result)
        self.assertIsNone(self.old.poll())

    def test_exited_unreaped_admission_does_not_count_as_live_work(self):
        child = subprocess.Popen(["true"])
        try:
            deadline = time.monotonic() + 5
            while (
                Path(f"/proc/{child.pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
                != "Z"
            ):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(0.01)
            self.assertNotIn(child.pid, support.descendants(os.getpid()))
        finally:
            child.wait(timeout=5)

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
