"""Isolated real-bundle acceptance; no live DB, model calls, or user credentials."""

import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import secrets
import select
import shutil
import socket
import sqlite3
import struct
import subprocess
import sys
import time
import start_binary as engine
import uuid

if sqlite3.sqlite_version_info < (3, 42):
    import pysqlite3 as sqlite3

from test_switch import SwitchTest
from source_isolation import deny_source
import start_support as support


def read_exact(stream, count):
    value = b""
    while len(value) < count:
        if not select.select([stream], [], [], 20)[0]:
            raise TimeoutError("Child protocol timeout")
        data = os.read(stream.fileno(), count - len(value))
        if not data:
            raise RuntimeError("Child exited before protocol reply")
        value += data
    return value


def runtime_children(fixture, source):
    current = json.loads((fixture.instance / "current.json").read_text())
    release = Path(current["release"])
    workspace = fixture.root / "agent-workspace"
    workspace.mkdir(exist_ok=True)
    env = {
        "PATH": str(release) + ":/usr/bin:/bin",
        "LVK_INSTANCE_DIR": str(fixture.instance),
        "MCP_HOST": "127.0.0.1",
        "MCP_PORT": str(fixture.port),
    }
    # Real host bootstrap verifies executable/runtime libraries and persistent
    # host protocol, without launching any provider or charging a model.
    host = subprocess.Popen(
        [str(release / "agent-process-host")],
        cwd=workspace,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        preexec_fn=lambda: deny_source(source),
    )
    try:
        bootstrap = {
            "protocol_version": 2,
            "run_attempt_id": str(uuid.uuid4()),
            "host_instance_id": str(uuid.uuid4()),
            "auth_token": secrets.token_hex(32),
            "requested_endpoint": "127.0.0.1:0",
        }
        data = json.dumps(bootstrap).encode()
        host.stdin.write(struct.pack(">I", len(data)) + data)
        host.stdin.flush()
        length = struct.unpack(">I", read_exact(host.stdout, 4))[0]
        ready = json.loads(read_exact(host.stdout, length))
        assert ready["host_pid"] == host.pid and ready["protocol_version"] == 2
        assert os.readlink(f"/proc/{host.pid}/cwd") == str(workspace)
    finally:
        host.terminate()
        host.wait(timeout=10)
        host.stdin.close()
        host.stdout.close()
    # Resolve the actual bundled MCP executable through the same inherited PATH
    # used for an Agent process. This does not claim a real provider invocation.
    mcp = subprocess.Popen(
        ["vibe-kanban-mcp"],
        cwd=workspace,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        preexec_fn=lambda: deny_source(source),
    )
    try:
        request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "isolated-test", "version": "1"},
            },
        }
        mcp.stdin.write(json.dumps(request).encode() + b"\n")
        mcp.stdin.flush()
        line = b""
        while not line.endswith(b"\n"):
            line += read_exact(mcp.stdout, 1)
        reply = json.loads(line)
        assert reply["id"] == 1 and "result" in reply, reply
        assert os.readlink(f"/proc/{mcp.pid}/exe") == str(release / "vibe-kanban-mcp")
        assert os.readlink(f"/proc/{mcp.pid}/cwd") == str(workspace)
    finally:
        mcp.terminate()
        mcp.wait(timeout=10)
        mcp.stdin.close()
        mcp.stdout.close()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--bundle", type=Path, required=True)
    p.add_argument(
        "--source",
        type=Path,
        required=True,
        help="Trusted migration test input; never runtime cwd",
    )
    a = p.parse_args()
    source = a.source.resolve()
    fixture = SwitchTest()
    fixture.setUp()
    try:
        fixture.old.terminate()
        fixture.old.wait(timeout=10)
        fixture.db.unlink()
        migrations = sorted((source / "crates/db/migrations").glob("*.sql"))
        assert len(migrations) > 1
        with contextlib.closing(sqlite3.connect(fixture.db)) as db, db:
            db.execute(
                "CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)"
            )
            for sql in migrations[:-1]:
                db.executescript("BEGIN;\n" + sql.read_text())
                version, description = sql.stem.split("_", 1)
                db.execute(
                    "INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(?,?,1,?,1)",
                    (
                        int(version),
                        description.replace("_", " "),
                        hashlib.sha384(sql.read_bytes()).digest(),
                    ),
                )
                db.commit()
        token = fixture.instance / "data/access-internal.token"
        token.write_text(secrets.token_urlsafe(32))
        token.chmod(0o600)
        fixture.write_run(
            fixture.builds[0],
            {
                "LVK_ACCESS_DEPLOYMENT": "local",
                "PREVIEW_PROXY_PORT": "0",
                "VK_ALLOWED_ORIGINS": f"http://127.0.0.1:{fixture.port}",
            },
        )
        launcher = fixture.instance / "switch/launcher.py"
        launcher.write_text(
            "from source_isolation import deny_source\ndeny_source("
            + repr(str(source))
            + ")\n"
            + launcher.read_text()
        )
        fixture.old = subprocess.Popen(
            [
                "/usr/bin/python3",
                str(fixture.instance / "switch/launcher.py"),
                str(fixture.instance),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            cwd=fixture.root,
        )
        fixture.wait_port()
        bundle = fixture.root / "real-bundle"
        shutil.copytree(a.bundle, bundle)
        fixture.builds[1] = {"bundle": str(bundle)}
        # Runtime children deny source. The engine remains unrestricted so its
        # required procfs identity/ptrace checks can observe other processes.
        # Independent Landlock domains cannot inspect each other's /proc/exe.
        forbidden = subprocess.run(
            [
                "/usr/bin/python3",
                "-c",
                'open(__import__("sys").argv[1]).read()',
                str(source / "Cargo.toml"),
            ],
            cwd=fixture.root,
            preexec_fn=lambda: deny_source(source),
            capture_output=True,
        )
        assert forbidden.returncode != 0 and b"PermissionError" in forbidden.stderr
        for label in ["migration_pending", "no_pending_migration"]:
            command = fixture.cli(
                extra=["--async"] if label == "no_pending_migration" else []
            )
            command[0] = "/usr/bin/python3"
            command[1] = str(fixture.instance / "switch/start_binary.py")
            result = subprocess.run(
                command,
                text=True,
                capture_output=True,
                timeout=180,
                cwd=fixture.root,
            )
            state = json.loads(result.stdout)
            if label == "no_pending_migration":
                assert result.returncode == 0 and state["status"] == "accepted", state
                request_id = state["operation_id"]
                deadline = time.monotonic() + 180
                while True:
                    state = engine.status(request_id, "operator")
                    if state["status"] in engine.TERMINAL:
                        break
                    assert (
                        time.monotonic() < deadline
                    ), "Independent worker did not settle"
                    time.sleep(0.1)
                assert engine.status(request_id, "operator") == state

            print(json.dumps({"case": label, "result": state}), flush=True)
            if state["status"] != "succeeded":
                # Preserve safe diagnostics only; arbitrary server logs stay private.
                raise AssertionError("Real LVK switch failed: " + str(state))
            op = Path(state["result_path"]).parent
            with contextlib.closing(
                sqlite3.connect(op / "data/db.v2.sqlite")
            ) as backup:
                before = backup.execute(
                    "SELECT count(*) FROM _sqlx_migrations"
                ).fetchone()[0]
            with contextlib.closing(sqlite3.connect(fixture.db)) as db:
                after = db.execute("SELECT count(*) FROM _sqlx_migrations").fetchone()[
                    0
                ]
            assert before == len(migrations) - (label == "migration_pending")
            assert after == len(migrations)
            runtime_children(fixture, source)
        print(
            "PASS: source denied; real UI asset hashes/API, startup migration/no migration, async worker survives real server exit and receipt reread, host bootstrap and bundled stdio MCP from Workspace cwd. Real provider/model/Auth0/browser interaction not exercised."
        )
    finally:
        fixture.tearDown()


if __name__ == "__main__":
    main()
