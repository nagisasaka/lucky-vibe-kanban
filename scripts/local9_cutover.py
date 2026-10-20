#!/usr/bin/env python3
"""One-time LOCAL-9 operator cutover. No live action without --execute and a TTY.
Pins are deliberately instance-specific. No Integration actions or DB restore.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import select
import shutil
import signal
import socket
import sqlite3
import subprocess
import time
import urllib.request
import uuid

ROOT = Path('/var/tmp/lvk-instances/dev')
SOURCE = ROOT / 'runtime/worktrees/d546-local-3-4-integr/lucky-vibe-kanban'
LIVE = Path('/var/tmp/vibe-kanban/worktrees/1f9a-chatgpt/lucky-vibe-kanban/target/debug/server')
SOURCE_SHA = '33587794d71f5da0107dd34b6eb6be5393511a70'
OLD = '3e37ca967d7faf0cdf0737568a8243b838b84b163fc18d68dd6152efde53c4d5'
NEW = 'efa3ff3a66f0eeb0a6efc8661efc280592b6a02cf72d9d60cf35a6442c1135d6'
PAYLOAD = '8158b60f43a4412d64b2f01e2696de717778dec2f1d3c4aaeda7167b5e88022e'
MAIN = 'b385a2d2d1a8683fbdcb8d931356d34f4264169c'
RUN = 'c68ce019-9349-4679-976f-326120191c08'
REFS = [('vk/57ca-codex-cli-plan-p', '7922bf778fa5837762f056bcf7322b5191d4d0ab'),
        ('vk/0abf-codex', '4a11c7a304891f1cfe95f9daecd4a5e319de51d0'),
        ('vk/1712-lvk-web-push-cod', 'b8f954948d615e66866bba85b5deb6882e71d55e'),
        ('vk/2085-codex-lvk-mcp-li', '40487d16a091b0f67eb24ecd011e507a5f42b8db')]
ASSET_DIGEST = '944540d7eb8081ac35cc53c06a9a86bf3f6f9bf53e470273ee17286efffbe599'
PINS = {'/var/tmp/lvk-instances/dev/start.sh': '821dcf686c8c95c9f25cf66006578882dfd725b5ac753895d56c56a5c3f4d734', '/var/tmp/lvk-instances/dev/run.py': '9cc7008b176be6b859fb7af0547b5ffa4dbf5785a5673cf40189abc2be15bfc1', '/var/tmp/vibe-kanban/worktrees/1f9a-chatgpt/lucky-vibe-kanban/target/debug/agent-process-host': 'c59b682595955ea6f5bdcd32c3229b9de3ea4d104b939deb99bbf33e59508458'}


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(path):
    with open(path, 'rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def elf(path):
    with open(path, 'rb') as f:
        require(f.read(4) == b'\x7fELF', 'Not an ELF executable')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def http(path, token=None, port=4020):
    headers = {} if token else {'Host': 'dev.luckyvibekanban.com'}
    if token:
        headers['X-LVK-Internal-Token'] = token
    req = urllib.request.Request(f'http://127.0.0.1:{port}{path}', headers=headers)
    with urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect).open(req, timeout=3) as response:
        return json.load(response)


class Cutover:
    def __init__(self):
        self.root, self.live = ROOT, LIVE
        self.candidate = SOURCE / 'target/debug/server'
        self.directory = Path('/var/tmp/local9-cutover-33587794')
        self.old, self.new = OLD, NEW
        self.port = 4020
        self.stop_timeout, self.start_timeout = 60, 45
        self.stopped = False
        self.switched = False
        self.env = None
        self.log = None
        self.launch_lock = None

    def record(self, stage):
        # Fixed stage names only: never serialize environment, HTTP body or exception text.
        message = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()) + ' ' + stage
        try:
            print(message, flush=True)
        except BrokenPipeError:
            pass
        if self.log:
            self.log.write(message + '\n'); self.log.flush(); os.fsync(self.log.fileno())
            tmp = self.directory / 'state.tmp'
            with tmp.open('w') as state:
                state.write(json.dumps({'stage': stage, 'time': time.time()}))
                state.flush(); os.fsync(state.fileno())
            os.replace(tmp, self.directory / 'state.json')

    def finish(self, stage, code):
        self.record(stage)
        with (self.directory / 'exit-code').open('w') as output:
            output.write(str(code) + '\n'); output.flush(); os.fsync(output.fileno())
        return code

    def pid(self):
        return int((self.root / 'server.pid').read_text().strip())

    def identity(self, pid, expected):
        require(pid > 1 and Path(f'/proc/{pid}').stat().st_uid == os.getuid(), 'PID owner mismatch')
        require(os.readlink(f'/proc/{pid}/exe') == str(self.live), 'PID executable mismatch')
        require(sha(f'/proc/{pid}/exe') == expected, 'PID binary mismatch')
        require(os.readlink(f'/proc/{pid}/cwd') == str(self.live.parent.parent.parent), 'PID cwd mismatch')
        return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]

    def alive(self, pid):
        try:
            return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[0] != 'Z'
        except FileNotFoundError:
            return False

    def listening(self):
        with socket.socket() as s:
            s.settimeout(1)
            return s.connect_ex(('127.0.0.1', self.port)) == 0

    def owns_listener(self, pid):
        inodes = set()
        for line in Path('/proc/net/tcp').read_text().splitlines()[1:]:
            fields = line.split()
            if fields[1] == f'0100007F:{self.port:04X}' and fields[3] == '0A':
                inodes.add('socket:[' + fields[9] + ']')
        links = set()
        for f in Path(f'/proc/{pid}/fd').iterdir():
            try:
                links.add(os.readlink(f))
            except FileNotFoundError:
                pass
        require(bool(inodes & links), 'Listener does not belong to verified PID')

    def frozen(self):
        for ref, expected in [('main', MAIN)] + REFS:
            actual = subprocess.check_output(['git', '-C', str(SOURCE), 'rev-parse', 'refs/heads/' + ref], timeout=5, text=True).strip()
            require(actual == expected, 'Frozen Git ref changed')
        with sqlite3.connect('file:' + str(self.root / 'data/db.v2.sqlite') + '?mode=ro', uri=True, timeout=2) as db:
            row = db.execute('SELECT status,payload FROM integration_runs WHERE id=?', (uuid.UUID(RUN).bytes,)).fetchone()
            require(row and row[0] == 'blocked', 'Integration not blocked')
            p = json.loads(row[1])
            require(hashlib.sha256(row[1].encode()).hexdigest() == PAYLOAD, 'Integration payload changed')
            require(p['skip_validation'] is True and p['published'] is False and not p['publication_intent'] and p['result_commit'] is None and p['base_commit'] == MAIN, 'Integration policy changed')
            require([(x['branch'], x['commit']) for x in p['sources']] == REFS, 'Source selection changed')

    def idle(self, server_pid):
        with sqlite3.connect('file:' + str(self.root / 'data/db.v2.sqlite') + '?mode=ro', uri=True, timeout=2) as db:
            checks = [
                "SELECT count(*) FROM agent_runs WHERE status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')",
                "SELECT count(*) FROM execution_processes WHERE status='running'",
                "SELECT count(*) FROM workflow_runs WHERE status NOT IN ('succeeded','failed','cancelled','canceled')",
                "SELECT count(*) FROM agent_process_registry WHERE registry_status!='exited'",
                "SELECT count(*) FROM resource_operations WHERE status NOT IN ('succeeded','failed','cancelled','canceled')",
                "SELECT count(*) FROM orchestration_outbox WHERE delivery_status!='delivered'",
                "SELECT count(*) FROM scheduled_tasks WHERE enabled=1",
                "SELECT count(*) FROM integration_runs WHERE status NOT IN ('blocked','succeeded','cancelled','failed')",
                "SELECT count(*) FROM orchestration_runs WHERE status NOT IN ('failed','succeeded','cancelled') AND NOT (id=x'" + uuid.UUID(RUN).hex + "' AND status='cancelling')",
            ]
            for query in checks:
                require(db.execute(query).fetchone()[0] == 0, 'Active job, unresolved delivery or enabled schedule; finish/resolve before cutover')
        # The one retained cancelling parent is inert only with all agent/host
        # executions exited and all its outbox receipts delivered, checked above.
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit() or int(proc.name) in (os.getpid(), server_pid):
                continue
            try:
                comm = (proc / 'comm').read_text().strip()
                require(comm not in ('cargo', 'rustc', 'agent-process-h', 'agent-process-host'), 'Concurrent build or agent host')
                if proc.stat().st_uid != os.getuid():
                    continue
                for fd in (proc / 'fd').iterdir():
                    try:
                        link = os.readlink(fd)
                    except FileNotFoundError:
                        continue
                    require(not link.startswith(str(self.root / 'data/db.v2.sqlite')), 'Other process has SQLite open')
            except (FileNotFoundError, ProcessLookupError):
                continue
            # Permission errors on same-uid process inspection are not ignored.

    def coordination(self):
        token = (self.root / 'data/access-internal.token').read_text().strip()
        data = http('/api/resource-coordination/snapshot', token)['data']
        require(not any(data[k] for k in ('resources', 'holders', 'queue')), 'Coordination changed; operator must use registered resource contract')

    def preflight(self):
        require(pwd.getpwuid(os.getuid()).pw_name == 'appuser', 'Run as appuser')
        require(not Path('/run/lvk-maintenance/active').exists(), 'Host maintenance active')
        require(sha(self.live) == self.old and sha(self.candidate) == self.new, 'Artifact hash changed')
        elf(self.live); elf(self.candidate)
        for path, digest in PINS.items():
            require(sha(path) == digest, 'Launcher or frontend asset changed')
        assets = SOURCE / 'packages/local-web/dist'
        rows = [(str(f.relative_to(assets)), sha(f)) for f in sorted(assets.rglob('*')) if f.is_file()]
        require(hashlib.sha256(json.dumps(rows).encode()).hexdigest() == ASSET_DIGEST, 'Frontend assets changed')
        self.frozen(); self.coordination()
        pid = self.pid(); self.identity(pid, self.old); self.owns_listener(pid)
        self.idle(pid)
        # Preserve the actually running environment, not an assumed SSH profile.
        raw = Path(f'/proc/{pid}/environ').read_bytes()
        self.env = dict(x.split(b'=', 1) for x in raw.split(b'\0') if x)
        require(self.env.get(b'LVK_INSTANCE_DIR') == str(self.root).encode(), 'Instance environment mismatch')
        require(shutil.disk_usage(self.directory.parent).free > 4 * 1024**3, 'Insufficient backup space')
        return pid

    def stop(self, pid, digest):
        fd = os.pidfd_open(pid)
        try:
            self.identity(pid, digest)  # pidfd prevents PID reuse between check and signal
            signal.pidfd_send_signal(fd, signal.SIGTERM)
            require(select.select([fd], [], [], self.stop_timeout)[0], 'Graceful stop timed out; no force kill')
        finally:
            os.close(fd)
        require(not self.listening(), 'Listener remains after stop')

    def backup_data(self):
        shutil.copytree(self.root / 'data', self.directory / 'data', symlinks=True)
        src = sqlite3.connect('file:' + str(self.root / 'data/db.v2.sqlite') + '?mode=ro', uri=True, timeout=2)
        dst = sqlite3.connect(self.directory / 'db.v2.consistent.sqlite')
        try:
            src.backup(dst, pages=128, sleep=0.05)
            require(dst.execute('PRAGMA integrity_check').fetchone()[0] == 'ok', 'Backup integrity failed')
        finally:
            dst.close(); src.close()

    def replace(self, source, digest):
        temp = self.live.with_name('server.local9-one-shot')
        with temp.open('xb') as output, open(source, 'rb') as inp:
            shutil.copyfileobj(inp, output); output.flush(); os.fsync(output.fileno())
        os.chmod(temp, 0o755)
        require(sha(temp) == digest, 'Replacement copy mismatch')
        os.replace(temp, self.live)
        fd = os.open(self.live.parent, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def start(self, digest):
        require(not self.listening(), 'Refuse second listener')
        if self.launch_lock:
            fcntl.flock(self.launch_lock, fcntl.LOCK_UN)
        server_log = self.root / 'logs/server.log'
        offset = server_log.stat().st_size if server_log.exists() else 0
        with (self.directory / 'launcher.log').open('ab') as log:
            subprocess.run([str(self.root / 'start.sh')], env=self.env, stdin=subprocess.DEVNULL,
                           stdout=log, stderr=log, timeout=10, check=True)
        deadline = time.monotonic() + self.start_timeout
        while time.monotonic() < deadline:
            try:
                pid = self.pid(); self.identity(pid, digest); self.owns_listener(pid)
                if http('/api/health', port=self.port).get('data') == 'OK':
                    with server_log.open('rb') as current_log:
                        current_log.seek(offset)
                        require(f'Main server on :{self.port}'.encode() in current_log.read(1024 * 1024), 'Startup log not confirmed')
                    return pid
            except (OSError, RuntimeError, ValueError):
                pass
            time.sleep(0.5)
        raise RuntimeError('Startup health timeout')

    def run(self):
        # Atomic private directory creation makes repeat execution refuse, even
        # after a failed or interrupted run. No automatic deletion/retry.
        self.directory.mkdir(mode=0o700)
        self.log = (self.directory / 'cutover.log').open('x')
        self.record('PREFLIGHT')
        try:
            pid = self.preflight()
            shutil.copy2(self.live, self.directory / 'server.old')
            require(sha(self.directory / 'server.old') == self.old, 'Old binary backup mismatch')
            with (self.directory / 'server.old').open('rb') as saved:
                os.fsync(saved.fileno())
            for name in ('start.sh', 'run.py', 'deployed-build.json'):
                shutil.copy2(self.root / name, self.directory / name)
            # Sensitive private recovery data; never print it.
            (self.directory / 'launch-environment.json').write_text(json.dumps({os.fsdecode(k): os.fsdecode(v) for k, v in self.env.items()}))
            self.record('BACKUP_BINARY_READY')
            self.launch_lock = (self.root / 'start.lock').open('rb')
            fcntl.flock(self.launch_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.frozen(); self.idle(pid); self.coordination()
            self.stop(pid, self.old); self.stopped = True
            self.record('STOPPED')
            self.idle(-1); self.frozen(); self.backup_data()
            self.record('BACKUP_DATA_READY')
            self.idle(-1); self.frozen()
            require(not self.listening(), 'Unexpected listener before switch')
            self.replace(self.candidate, self.new); self.switched = True
            self.record('SWITCHED')
            newpid = self.start(self.new)
            self.frozen(); self.idle(newpid)
            return self.finish('SUCCESS_NO_RESUME', 0)
        except Exception as error:
            self.record('FAILED_' + type(error).__name__)
            if type(error) is RuntimeError:
                self.record(str(error))
            if not self.stopped:
                return self.finish('ABORT_OR_STOP_UNCONFIRMED_OPERATOR_CHECK', 2)
            try:
                # A new PID is only signalled after exe/hash/owner verification.
                # If ownership is ambiguous, fail closed instead of overwriting.
                pid = self.pid()
                if self.alive(pid):
                    self.stop(pid, self.new if self.switched else self.old)
                require(not self.listening(), 'Unknown listener; manual recovery required')
                self.replace(self.directory / 'server.old', self.old)
                self.start(self.old)
                self.frozen()
                return self.finish('ROLLED_BACK_NO_DB_RESTORE', 3)
            except Exception:
                return self.finish('MANUAL_RECOVERY_REQUIRED_NO_FORCE_KILL', 4)
        finally:
            if self.launch_lock:
                self.launch_lock.close()
            self.log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    require(args.execute and os.isatty(0), 'Independent operator SSH TTY and --execute required')
    # Explicitly reject invocation from the server/agent ancestry.
    pid = os.getppid()
    while pid > 1:
        comm = Path(f'/proc/{pid}/comm').read_text().strip()
        require(not any(x in comm.lower() for x in ('codex', 'agent-process', 'server')), 'Do not self-cutover from an agent')
        pid = int(Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[1])
    require(all(os.readlink('/proc/self/ns/' + key) == value for key, value in {'mnt': 'mnt:[4026532426]', 'pid': 'pid:[4026532542]'}.items()), 'Container namespace changed; re-review preparation')
    os.umask(0o077)
    # Whole-program deadline; no force-kill fallback. Interrupted state is durable.
    signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError('Cutover deadline')))
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda *_: (_ for _ in ()).throw(RuntimeError('Operator interruption')))
    signal.alarm(300)
    raise SystemExit(Cutover().run())


if __name__ == '__main__':
    main()
