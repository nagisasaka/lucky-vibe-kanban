#!/usr/bin/env python3
"""Test-only lossless stdout tee around the unmodified Codex app-server.
The fixture supplies LVK_FIXTURE_WIRE and launches this in its managed group.
"""
import os
import signal
import subprocess
import sys
import threading

process = subprocess.Popen([os.environ.get('LVK_FIXTURE_CODEX', 'codex'), *sys.argv[1:]],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE)

def stop(*_):
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=10)
    sys.exit(0)

signal.signal(signal.SIGTERM, stop)

def forward():
    try:
        for line in sys.stdin.buffer:
            process.stdin.write(line)
            process.stdin.flush()
    except (BrokenPipeError, ValueError):
        pass

threading.Thread(target=forward, daemon=True).start()
with open(os.environ['LVK_FIXTURE_WIRE'], 'ab', buffering=0) as wire:
    for line in process.stdout:
        wire.write(line)
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
sys.exit(process.wait())
