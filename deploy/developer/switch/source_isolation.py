"""Test-only Landlock read/execute denial for a build source tree (Linux x86_64).

Restrict only the child, never rename or chmod the real repository. No privilege
or mount namespace is required. A kernel without Landlock fails the test closed.
"""

import ctypes
import os
from pathlib import Path


def deny_source(source):
    libc = ctypes.CDLL(None, use_errno=True)

    class Ruleset(ctypes.Structure):
        _fields_ = [("handled_access_fs", ctypes.c_uint64)]

    class Beneath(ctypes.Structure):
        _pack_ = 1
        _fields_ = [("allowed_access", ctypes.c_uint64), ("parent_fd", ctypes.c_int32)]

    access = 1 | 4 | 8  # EXECUTE, READ_FILE, READ_DIR
    rules = Ruleset(access)
    fd = libc.syscall(444, ctypes.byref(rules), ctypes.sizeof(rules), 0)
    if fd < 0:
        raise OSError(ctypes.get_errno(), "Landlock unavailable")
    try:
        forbidden = Path(source).resolve()
        assert forbidden != Path("/")
        cursor = Path("/")
        for component in forbidden.parts[1:]:
            next_path = cursor / component
            for sibling in cursor.iterdir():
                if sibling == next_path or sibling.is_symlink():
                    continue
                try:
                    handle = os.open(sibling, os.O_PATH | os.O_CLOEXEC)
                except PermissionError:
                    continue
                try:
                    allowed = access if sibling.is_dir() else 1 | 4
                    rule = Beneath(allowed, handle)
                    if libc.syscall(445, fd, 1, ctypes.byref(rule), 0) != 0:
                        raise OSError(ctypes.get_errno(), "Landlock rule failed")
                finally:
                    os.close(handle)
            cursor = next_path
        if libc.prctl(38, 1, 0, 0, 0) != 0 or libc.syscall(446, fd, 0) != 0:
            raise OSError(ctypes.get_errno(), "Landlock restriction failed")
    finally:
        os.close(fd)
