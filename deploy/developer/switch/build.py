"""Build pinned dev artifacts, independently of live instances and the switch engine."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

import bundle
import frontend_bundle as frontend
import start_support as s


def _build(source, output, mode, reason):
    source = source.resolve(strict=True)
    s.need(reason.strip(), 'Record affected inputs and compatibility decision; use both if uncertain')
    def git(*args):
        return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()
    sha = git('rev-parse', 'HEAD')
    s.need(not git('status', '--porcelain', '--untracked-files=all'), 'Use a clean pinned checkout')
    output.mkdir(mode=0o700)
    env = dict(os.environ, LVK_BUILD_REVISION=sha, LVK_BUILD_KIND='development', LVK_BUILD_DIRTY='false')
    report = {'revision': sha, 'mode': mode, 'decision': reason, 'commands': [], 'seconds': {}}
    def run(name, command):
        start = time.monotonic()
        report['commands'].append(command)
        subprocess.run(command, cwd=source, env=env, check=True)
        report['seconds'][name] = time.monotonic() - start
    try:
        if mode in ('frontend', 'both'):
            dist = output / 'dist'
            run('frontend_build', ['pnpm', '--dir', 'packages/local-web', 'run', 'build',
                '--outDir', str(dist), '--base', frontend.PREFIX + sha + '/', '--manifest'])
            frontend.package(dist, sha, output / 'frontend')
            shutil.rmtree(dist)
        if mode in ('backend', 'both'):
            # Cargo's dependency/fingerprint graph decides which binaries need
            # recompilation. Do not guess compatibility from a path whitelist or
            # copy an auxiliary binary from an unrelated checkout.
            run('backend_build', ['node', 'scripts/dev-cargo.mjs', '--purpose', 'dev', 'build', '--locked',
                '-p', 'server', '-p', 'local-deployment', '-p', 'mcp',
                '--bin', 'server', '--bin', 'agent-process-host', '--bin', 'vibe-kanban-mcp',
                '--features', 'server/developer-runtime'])
            target = source / 'target/release'
            manifest = bundle.package(target / 'server', target / 'agent-process-host',
                target / 'vibe-kanban-mcp', output / 'backend',
                output / 'frontend' if mode == 'both' else None)
            report['artifacts'] = manifest['files']
            provenance = {'validated_against_revision': sha, 'original_build_revision': None,
                'policy': 'Cargo freshness using the same locked checkout and dev profile; original compilation commit is not inferred from reuse; no external binary reuse'}
            report['auxiliary_provenance'] = provenance
            manifest['auxiliary_provenance'] = provenance
            s.atomic(output / 'backend/bundle.json', json.dumps(manifest).encode())
        s.need(git('rev-parse', 'HEAD') == sha and not git('status', '--porcelain', '--untracked-files=all'),
               'Source changed during build; discard candidate')
        report['status'] = 'succeeded'
    except BaseException:
        report['status'] = 'failed'
        raise
    finally:
        s.save(output / 'build.json', report)
    return report


def build(source, output, mode, reason):
    import uuid
    s.need(not output.exists(), 'Candidate output already exists')
    pending = output.with_name('.pending-' + output.name + '-' + uuid.uuid4().hex)
    report = _build(source, pending, mode, reason)
    # Publish the candidate only after all builds and the final pinned-source
    # check succeed. A failed build remains a diagnostic .pending directory.
    os.rename(pending, output)
    return report


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--mode', choices=['frontend', 'backend', 'both'], default='both')
    p.add_argument('--reason', required=True)
    a = p.parse_args()
    os.umask(0o077)
    print(json.dumps(build(a.source, a.output.absolute(), a.mode, a.reason)))
