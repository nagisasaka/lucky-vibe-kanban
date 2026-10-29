"""Real stdio binary -> installed common engine, isolated C server/DB only."""
import json
import contextlib
import os
from pathlib import Path
import select
import subprocess
import sys
import time
import unittest
import uuid
import test_switch
import start_binary as engine


class StdioRestartTest(unittest.TestCase):
    def setUp(self):
        binary = os.environ.get("LVK_TEST_MCP_BINARY")
        if not binary: self.skipTest("set LVK_TEST_MCP_BINARY to the newly built stdio binary")
        self.binary = str(Path(binary).resolve())
        self.fixture = test_switch.SwitchTest()
        self.fixture.setUp()
        self.addCleanup(self.cleanup_fixture)
        self.project = uuid.uuid4()
        with engine.s.ro(self.fixture.db) as _:
            pass
        import sqlite3
        with contextlib.closing(sqlite3.connect(self.fixture.db)) as db, db:
            db.execute("CREATE TABLE local_workspace_links(workspace_id BLOB,issue_id BLOB)")
            db.execute("CREATE TABLE local_issues(id BLOB,project_id BLOB)")
            db.execute("CREATE TABLE projects(id BLOB PRIMARY KEY)")
            db.execute("INSERT INTO projects VALUES(?)", (self.project.bytes,))
            db.execute("CREATE TABLE runtime_access_settings(id INTEGER PRIMARY KEY,revision INTEGER,settings TEXT)")
            db.execute("INSERT INTO runtime_access_settings VALUES(1,1,?)", (json.dumps({"execution_enabled": True}),))
        worker = self.fixture.instance / "switch/start_binary.py"
        worker.write_text(worker.read_text().replace("BUSY_RECHECK_SECONDS = 0.01", "BUSY_RECHECK_SECONDS = 1"))

    def cleanup_fixture(self):
        # A failed assertion must still let our durable worker settle before teardown.
        deadline = time.monotonic() + 15
        root = self.fixture.instance / "switch-operations"
        while root.exists():
            states = [json.loads(p.read_text()) for p in root.glob("*/result.json")]
            if all(s["status"] in engine.TERMINAL for s in states): break
            if time.monotonic() >= deadline: break
            time.sleep(.05)
        self.fixture.tearDown()

    def tool(self, name, arguments):
        env = {"PATH":os.environ.get("PATH", "/usr/bin:/bin"),"MCP_HOST":"127.0.0.1","MCP_PORT":str(self.fixture.port),"LVK_INSTANCE_DIR":str(self.fixture.instance),"RUST_LOG":"error"}
        proc = subprocess.Popen([self.binary], cwd=self.fixture.root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0)
        try:
            messages=[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"isolated-restart","version":"1"}}},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}]
            proc.stdin.write((''.join(json.dumps(m)+'\n' for m in messages)).encode()); proc.stdin.flush()
            pending=b""; deadline=time.monotonic()+15; reply=None
            while reply is None:
                self.assertGreater(deadline,time.monotonic(), "stdio reply timeout")
                self.assertTrue(select.select([proc.stdout],[],[],deadline-time.monotonic())[0])
                chunk=os.read(proc.stdout.fileno(),65536); self.assertTrue(chunk)
                pending+=chunk
                while b"\n" in pending:
                    line,pending=pending.split(b"\n",1)
                    value=json.loads(line)
                    if value.get("id")==2: reply=value
            proc.stdin.close(); proc.wait(timeout=5)
            return reply["result"]
        finally:
            if proc.poll() is None: proc.terminate(); proc.wait(timeout=5)
            if not proc.stdin.closed: proc.stdin.close()
            proc.stdout.close()

    def args(self, request=None):
        return {"project_id":str(self.project),"request_id":str(request or uuid.uuid4()),"purpose":"isolated stdio switch","bundle":self.fixture.builds[1]["bundle"],"no_new_work":True}

    def value(self, result):
        return json.loads(result["content"][0]["text"])

    def test_real_stdio_admits_duplicates_and_common_engine_completes(self):
        args=self.args(); started=time.monotonic()
        first=self.tool("self_restart",args)
        self.assertNotEqual(first.get("isError"),True, first)
        admitted=self.value(first)
        self.assertEqual(admitted["operation_id"],args["request_id"])
        self.assertLess(time.monotonic()-started,10)  # never waits the production 30 seconds
        repeated=self.tool("self_restart",args)
        self.assertEqual(self.value(repeated)["operation_id"],args["request_id"])
        owner=engine.stdio_owner(str(self.project))
        deadline=time.monotonic()+10
        while True:
            status=engine.status(args["request_id"],owner)
            if status["status"] in engine.TERMINAL: break
            self.assertLess(time.monotonic(),deadline); time.sleep(.05)
        self.assertEqual(status["status"],"succeeded",status)
        # Read the real completed engine result with the legitimate unsaved state.
        import sqlite3
        with contextlib.closing(sqlite3.connect(self.fixture.db)) as db, db:
            db.execute("DELETE FROM runtime_access_settings")
        read=self.tool("get_self_restart_operation",{k:args[k] for k in ["project_id","request_id"]})
        self.assertEqual(self.value(read)["status"],"succeeded")
        changed={**args,"purpose":"different"}
        self.assertTrue(self.tool("self_restart",changed)["isError"])
        ops=list((self.fixture.instance/"switch-operations").iterdir())
        self.assertEqual(len(ops),1)

    def test_runtime_revocation_during_busy_wait_prevents_stop(self):
        holder=subprocess.Popen(["/bin/sleep","15"],env={"LVK_INSTANCE_DIR":str(self.fixture.instance)},stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        try:
            args=self.args()
            admitted=self.tool("self_restart",args)
            self.assertNotEqual(admitted.get("isError"),True,admitted)
            import sqlite3
            with contextlib.closing(sqlite3.connect(self.fixture.db)) as db, db:
                db.execute("UPDATE runtime_access_settings SET settings=?", (json.dumps({"execution_enabled":False}),))
            holder.terminate(); holder.wait(timeout=5)
            deadline=time.monotonic()+10
            while True:
                state=engine.status(args["request_id"],engine.stdio_owner(str(self.project)))
                if state["status"] in engine.TERMINAL: break
                self.assertLess(time.monotonic(),deadline); time.sleep(.05)
            self.assertEqual(state["status"],"failed",state)
            self.assertEqual(state["reason"],"Program execution disabled")
            self.assertIsNone(self.fixture.old.poll())
        finally:
            if holder.poll() is None: holder.terminate(); holder.wait(timeout=5)

    def test_disabled_runtime_and_coordination_are_rejected(self):
        args=self.args(); args["no_new_work"]=False
        self.assertTrue(self.tool("self_restart",args)["isError"])
        import sqlite3
        with contextlib.closing(sqlite3.connect(self.fixture.db)) as db, db:
            db.execute("UPDATE runtime_access_settings SET settings=?", (json.dumps({"execution_enabled":False}),))
        args["no_new_work"]=True
        self.assertTrue(self.tool("self_restart",args)["isError"])
        self.assertFalse((self.fixture.instance/"switch-operations").exists())

    def test_unsaved_runtime_status_reads_receipt_without_saving_defaults(self):
        import sqlite3
        with contextlib.closing(sqlite3.connect(self.fixture.db)) as db, db:
            db.execute("DELETE FROM runtime_access_settings")
        args = self.args()
        status = {key: args[key] for key in ["project_id", "request_id"]}
        # Unknown remains refused, but must reach status lookup rather than the guard.
        unknown = self.tool("get_self_restart_operation", status)
        self.assertTrue(unknown["isError"], unknown)
        self.assertNotIn("Program execution disabled", json.dumps(unknown))
        self.assertFalse((self.fixture.instance / "switch-operations").exists())
        op = self.fixture.instance / "switch-operations" / args["request_id"]
        op.mkdir(parents=True)
        owner = engine.stdio_owner(str(self.project))
        (op / "request.json").write_text(json.dumps({"owner": owner, "arguments": {}}))
        receipt = {"operation_id": args["request_id"], "status": "succeeded", "stage": "complete"}
        (op / "result.json").write_text(json.dumps(receipt))
        reply = self.tool("get_self_restart_operation", status)
        self.assertNotEqual(reply.get("isError"), True, reply)
        value = json.loads(reply["content"][0]["text"])
        self.assertEqual(value, receipt)
        with contextlib.closing(sqlite3.connect(self.fixture.db)) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM runtime_access_settings").fetchone()[0], 0)
        self.assertIsNone(self.fixture.old.poll())

    def test_unknown_result_and_other_owner_are_never_success(self):
        args=self.args(); status={k:args[k] for k in ["project_id","request_id"]}
        self.assertTrue(self.tool("get_self_restart_operation",status)["isError"])
        op=self.fixture.instance/"switch-operations"/args["request_id"]; op.mkdir(parents=True)
        (op/"request.json").write_text(json.dumps({"owner":"remote-oauth-owner","arguments":{}}))
        (op/"result.json").write_text(json.dumps({"status":"failed","stage":"fixture"}))
        self.assertTrue(self.tool("get_self_restart_operation",status)["isError"])
        self.assertEqual(json.loads((op/"request.json").read_text())["owner"],"remote-oauth-owner")

    def test_project_and_workspace_binding_are_checked_read_only(self):
        args=self.args(); args["project_id"]=str(uuid.uuid4())
        self.assertTrue(self.tool("self_restart",args)["isError"])
        command=[sys.executable,str(self.fixture.instance/"switch/start_binary.py"),"--instance",str(self.fixture.instance),"--stdio-project",str(self.project),"--stdio-port",str(self.fixture.port),"--stdio-workspace",str(uuid.uuid4()),"--status","--request-id",str(uuid.uuid4())]
        result=subprocess.run(command,capture_output=True,text=True,timeout=5)
        self.assertEqual(json.loads(result.stdout)["status"],"refused")
        self.assertFalse((self.fixture.instance/"switch-operations").exists())


if __name__ == "__main__": unittest.main()
