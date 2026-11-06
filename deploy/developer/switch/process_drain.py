"""Stop only a pidfd-bound server's proven child tree, never process groups."""
import contextlib
import json
import os
import select
import signal
import time

import start_support as s


def processes():
    rows = {}
    for path in s.P('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            rows[int(path.name)] = dict(
                state=fields[0], parent=int(fields[1]), group=int(fields[2]),
                session=int(fields[3]), start=fields[19], uid=path.stat().st_uid,
            )
        except (FileNotFoundError, ProcessLookupError):
            pass
    return rows


def stable(a, b):
    return a['start'] == b['start'] and a['uid'] == b['uid']


def descendants(rows, pid, seeds=()):
    found = {pid, *seeds}
    while True:
        expanded = found | {p for p, row in rows.items() if row['parent'] in found}
        if expanded == found:
            return {p for p in found - {pid} if rows[p]['state'] != 'Z'}
        found = expanded


def boundary(pid, row, owner):
    s.need(row['uid'] == owner, f'Child owner mismatch: PID {pid}')
    env, _ = s.process_files(s.P(f'/proc/{pid}'))
    instance = env.get(b'LVK_INSTANCE_DIR')
    s.need(instance is None or instance == str(s.DEV).encode(),
           f'Child belongs to another instance: PID {pid}')


def terminate(want, server_fd, operation=None, timeout=10):
    """Freeze the source/tree before KILL, then TERM and resume the server.

    SIGSTOP closes the child-fork window while the tree is pinned. The server
    still gets its existing graceful TERM/timeout; no server KILL is introduced.
    On refusal, resume only processes stopped here. A pinned child remains owned
    even when an exiting parent reparents it. Numeric PIDs never receive signals.
    """
    pid = want['pid']
    handles = {}
    paused = set()
    server_paused = False
    deadline = time.monotonic() + timeout

    def audit():
        if operation is not None:
            s.atomic(operation / 'child-stop.json', json.dumps({
                'server': want, 'processes': {str(p): row for p, (_, row) in handles.items()},
            }).encode())

    def capture(rows):
        known = set(handles)
        active = set()
        for child, (fd, row) in handles.items():
            if not select.select([fd], [], [], 0)[0]:
                s.need(child in rows and stable(row, rows[child]), 'Child PID identity changed')
                active.add(child)
        targets = descendants(rows, pid, active)
        s.need(os.getpid() not in targets, 'Switch worker is inside target tree; cannot stop itself')
        for child in targets:
            boundary(child, rows[child], rows[pid]['uid'])
        for child in sorted(targets - known):
            handles[child] = (os.pidfd_open(child), rows[child])
        current = processes()
        s.need(pid in current and stable(rows[pid], current[pid]), 'Server identity changed')
        for child in targets - known:
            fd, row = handles[child]
            if select.select([fd], [], [], 0)[0]:
                continue
            now = current.get(child)
            s.need(now is not None and stable(row, now), 'Child PID identity changed')
            parent = now['parent']
            s.need(parent == row['parent'] and parent in current and parent in rows
                   and stable(rows[parent], current[parent]), 'Child ancestry changed before capture')
        audit()
        return targets

    try:
        rows = processes()
        s.need(pid != os.getpid() and s.same(s.identity(pid), want) and rows[pid]['uid'] == os.getuid(),
               'Server identity/owner changed before child stop')
        s.need(rows[pid]['state'] not in ('T', 't'), 'Server was already stopped')
        capture(rows)  # Validate all boundaries before the first signal.
        signal.pidfd_send_signal(server_fd, signal.SIGSTOP)
        server_paused = True
        while True:
            rows = processes()
            s.need(s.same(s.identity(pid), want), 'Server identity changed during child stop')
            targets = capture(rows)
            for child in targets:
                fd, _ = handles[child]
                if not select.select([fd], [], [], 0)[0] and rows[child]['state'] not in ('T', 't'):
                    signal.pidfd_send_signal(fd, signal.SIGSTOP)
                    paused.add(child)
            frozen = processes()
            if (frozen[pid]['state'] == 'T' and targets == descendants(frozen, pid, {p for p in targets if p in frozen})
                    and all(frozen.get(p, {}).get('state') in ('T', 't', 'Z') for p in targets)):
                break
            s.need(time.monotonic() < deadline, 'Child freeze timeout; no cutover')
            time.sleep(.01)
        # The source and every live child are now frozen. Neither can fork while
        # we signal the captured tree. No PGID/session/name/cwd kill selection.
        s.pre_stop_writers(pid, allowed_pids=handles.keys())
        for child, (fd, row) in handles.items():
            if not select.select([fd], [], [], 0)[0]:
                now = processes().get(child)
                s.need(now is not None and stable(row, now), 'Pinned child identity changed')
                boundary(child, now, rows[pid]['uid'])
        for fd, _ in handles.values():
            if not select.select([fd], [], [], 0)[0]:
                signal.pidfd_send_signal(fd, signal.SIGKILL)
        for fd, _ in handles.values():
            s.need(bool(select.select([fd], [], [], max(0, deadline - time.monotonic()))[0]),
                   'Child KILL timeout; no backup/cutover, inspect child-stop.json')
        s.need(not descendants(processes(), pid), 'New child remains; no cutover')
        s.pre_stop_writers(pid)
        s.need(s.same(s.identity(pid), want), 'Server identity changed before TERM')
        signal.pidfd_send_signal(server_fd, signal.SIGTERM)
    finally:
        for child in paused:
            fd = handles[child][0]
            if not select.select([fd], [], [], 0)[0]:
                with contextlib.suppress(ProcessLookupError):
                    signal.pidfd_send_signal(fd, signal.SIGCONT)
        if server_paused:
            with contextlib.suppress(ProcessLookupError):
                signal.pidfd_send_signal(server_fd, signal.SIGCONT)
        for fd, _ in handles.values():
            os.close(fd)
