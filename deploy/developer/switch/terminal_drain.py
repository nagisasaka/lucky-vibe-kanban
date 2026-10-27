"""Linux developer PTY drain. No child-process exemptions in the stop guards."""
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
                session=int(fields[3]), tty=int(fields[4]), start=fields[19],
                uid=path.stat().st_uid,
            )
        except (FileNotFoundError, ProcessLookupError):
            pass
    return rows


def masters(pid):
    indexes = set()
    for fd in s.P(f'/proc/{pid}/fdinfo').iterdir():
        try:
            for line in fd.read_text().splitlines():
                if line.startswith('tty-index:'):
                    indexes.add(int(line.split()[1]))
        except FileNotFoundError:
            pass
    return {os.stat(f'/dev/pts/{index}').st_rdev for index in indexes}


def owned_roots(pid, rows):
    devices = masters(pid)
    roots = set()
    for child, row in rows.items():
        if row['state'] == 'Z' or row['parent'] != pid:
            continue
        if not (row['session'] == child and row['group'] == child
                and row['tty'] in devices and row['uid'] == rows[pid]['uid']):
            continue
        env, _ = s.process_files(s.P(f'/proc/{child}'))
        if (env.get(b'VIBE_KANBAN_TERMINAL') == b'1'
                and env.get(b'LVK_INSTANCE_DIR') == str(s.DEV).encode()):
            s.need(os.stat(f'/proc/{child}/fd/0').st_rdev == row['tty'],
                   'PTY shell stdin differs from its controlling terminal')
            roots.add(child)
    return roots


def members(rows, roots):
    found = set(roots)
    while True:
        expanded = found | {pid for pid, r in rows.items() if r['parent'] in found}
        if expanded == found:
            break
        found = expanded
    # A session member reparented outside the tree cannot be safely attributed.
    s.need(not any(r['state'] != 'Z' and r['session'] in roots and pid not in found
                   for pid, r in rows.items()), 'PTY session escaped its owned tree')
    for pid in found:
        row = rows[pid]
        root = rows.get(row['session'])
        s.need(row['session'] in roots and root is not None
               and row['uid'] == root['uid'] and row['tty'] == root['tty'],
               'PTY descendant changed ownership/session/TTY boundary')
        # Each job group must be entirely inside this owned session/tree.
        s.need(not any(r['state'] != 'Z' and r['group'] == row['group'] and other not in found
                       for other, r in rows.items()), 'PTY process group crosses ownership boundary')
    return {pid for pid in found if rows[pid]['state'] != 'Z'}


def stable(a, b):
    return all(a[k] == b[k] for k in ('parent', 'group', 'session', 'tty', 'start', 'uid'))


def terminate(want, operation, timeout=10):
    pid = want['pid']
    s.need(s.same(s.identity(pid), want), 'Server identity changed before PTY drain')
    original = processes()
    roots = owned_roots(pid, original)
    handles = {}
    deadline = time.monotonic() + timeout
    try:
        targets = members(original, roots)
        for child in sorted(targets):
            fd = os.pidfd_open(child)
            handles[child] = (fd, original[child])
        rows = processes()
        s.need(s.same(s.identity(pid), want), 'Server identity changed during PTY drain')
        s.need(owned_roots(pid, rows) == roots, 'PTY ownership changed during drain')
        s.need(members(rows, roots) == targets, 'PTY tree changed before termination')
        for child, (_, row) in handles.items():
            s.need(child in rows and stable(row, rows[child]), 'PTY PID identity changed')
            s.need(rows[child]['state'] not in ('T', 't'), 'PTY was already stopped; operator inspection required')
        s.save(operation / 'terminal-drain.json', {'server': want, 'processes': {
            str(p): row for p, (_, row) in handles.items()}})
        def depth(child):
            return 1 + depth(original[child]['parent']) if child in targets else 0
        jobs = sorted(targets - roots, key=depth, reverse=True)
        # Interactive shells ignore TERM by design. Terminate jobs first, then
        # log out their owning shell with HUP. There is one pass, no respawn loop.
        for child in jobs:
            fd, row = handles[child]
            if not select.select([fd], [], [], 0)[0]:
                now = processes().get(child)
                if select.select([fd], [], [], 0)[0]:
                    continue
                s.need(now is not None and stable(row, now), 'PTY job identity changed')
                signal.pidfd_send_signal(fd, signal.SIGTERM)
        for child in jobs:
            s.need(bool(select.select([handles[child][0]], [], [], max(0, deadline - time.monotonic()))[0]),
                   'PTY TERM timeout; no SIGKILL, inspect terminal-drain.json')
        for child in roots:
            # The shell may have exited naturally when its last job ended.
            fd = handles[child][0]
            if not select.select([fd], [], [], 0)[0]:
                now = processes().get(child)
                s.need(now is not None and stable(handles[child][1], now), 'PTY shell identity changed')
                signal.pidfd_send_signal(fd, signal.SIGHUP)
            s.need(bool(select.select([fd], [], [], max(0, deadline - time.monotonic()))[0]),
                   'PTY shell logout timeout; no SIGKILL')
        # Legacy servers do not wait() the shell. pidfd readiness confirms exit;
        # existing guards already disregard zombies (no execution or open FDs).
        s.need(not any(r['state'] != 'Z' and r['session'] in roots for r in processes().values()),
               'Live PTY session member remains')
    finally:
        for fd, _ in handles.values():
            os.close(fd)
