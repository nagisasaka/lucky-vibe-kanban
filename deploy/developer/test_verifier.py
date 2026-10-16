"""The installed host kit must work without a source checkout or a special cwd."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class InstalledVerifierTests(unittest.TestCase):
    def test_disposable_verifier_finds_its_helper_outside_repository(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            kit = root / "installed kit"
            kit.mkdir()
            source = Path(__file__).parent
            for name in ("verify-image.sh", "verify-access.mjs"):
                shutil.copy2(source / name, kit / name)
            binary = root / "bin"
            binary.mkdir()
            docker = binary / "docker"
            docker.write_text("""#!/usr/bin/env python3
import pathlib, sys
args = sys.argv[1:]
if args[0] == 'inspect':
    print('healthy')
elif args[0] == 'cp':
    assert pathlib.Path(args[1]).is_file(), 'Missing installed setup verifier'
elif args[0] == 'run' and '-i' in args:
    sys.stdin.read()
    print('test:fixture')
""")
            docker.chmod(0o755)
            subprocess.run(
                ["bash", str(kit / "verify-image.sh"), "example/lvk@sha256:fixture"],
                cwd=root,
                env={**os.environ, "PATH": str(binary) + os.pathsep + os.environ["PATH"]},
                check=True, capture_output=True, text=True, timeout=15,
            )
