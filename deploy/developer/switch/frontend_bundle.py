"""Immutable dev frontend artifacts. No processes, database or authentication changes."""

from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shutil
from urllib.parse import urlsplit

import start_support as s


PREFIX = '/__lvk_frontend/'


def revision(value):
    s.need(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', value),
           'Frontend revision must be a full lowercase commit SHA')
    return value


def safe_name(name):
    s.need(isinstance(name, str) and name and '\\' not in name
           and not name.startswith('/')
           and all(p not in ('', '.', '..') for p in name.split('/')),
           'Invalid frontend asset path')
    return name


class Entry(HTMLParser):
    def __init__(self):
        super().__init__()
        self.urls = []
        self.modules = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'script' and attrs.get('src'):
            self.urls.append(attrs['src'])
            if attrs.get('type') == 'module':
                self.modules.append(attrs['src'])
        if tag == 'link' and attrs.get('href'):
            self.urls.append(attrs['href'])


def inventory(root):
    s.need(root.is_dir() and root.resolve() == root, 'Frontend directory alias refused')
    files = {}
    for p in root.rglob('*'):
        s.need(not p.is_symlink(), 'Frontend symlinks refused')
        if p.is_dir():
            continue
        s.need(p.is_file(), 'Frontend special files refused')
        name = safe_name(p.relative_to(root).as_posix())
        if name != 'frontend.json':
            files[name] = s.digest(p)
    return files


def validate(root, manifest):
    s.need(manifest.get('format') == 1, 'Unknown frontend format')
    sha = revision(manifest.get('revision'))
    files = manifest['files']
    s.need(files == inventory(root), 'Frontend asset hash mismatch')
    s.need('index.html' in files and '.vite/manifest.json' in files,
           'Build with Vite --manifest; entry and graph required')
    for name in files:
        safe_name(name)
    graph = s.readjson(root / '.vite/manifest.json')
    s.need(isinstance(graph, dict) and graph, 'Empty Vite graph')
    entries = set()
    for item in graph.values():
        s.need(item['file'] in files, 'Missing Vite chunk')
        if item.get('isEntry'):
            entries.add(item['file'])
        for name in item.get('css', []) + item.get('assets', []):
            s.need(name in files, 'Missing Vite asset')
        for name in item.get('imports', []) + item.get('dynamicImports', []):
            s.need(name in graph, 'Missing Vite dependency')
    html = Entry()
    html.feed((root / 'index.html').read_text())
    base = PREFIX + sha + '/'
    s.need(html.modules, 'Missing module entry')
    for url in html.urls:
        path = urlsplit(url).path
        s.need(url.startswith(base) and path[len(base):] in files,
               'Entry must reference existing versioned local assets')
    s.need(all(urlsplit(url).path[len(base):] in entries for url in html.modules),
           'Entry differs from Vite graph')
    return manifest


def package(dist, sha, output):
    sha = revision(sha)
    # Validate before creating anything at the destination. Revalidate the copy.
    manifest = {'format': 1, 'revision': sha, 'files': inventory(dist)}
    validate(dist, manifest)
    s.need(not output.exists(), 'Frontend output already exists')
    try:
        shutil.copytree(dist, output, symlinks=True)
        validate(output, manifest)
        s.save(output / 'frontend.json', manifest)
        for p in output.rglob('*'):
            p.chmod(0o500 if p.is_dir() else 0o400)
        output.chmod(0o500)
    except BaseException:
        # An incomplete destination is never selectable. Preserve it for diagnosis.
        raise
    return manifest


def inspect(root):
    return validate(root, s.readjson(root / 'frontend.json'))


def install(instance, source):
    manifest = inspect(source)
    root = instance / 'frontend-releases'
    root.mkdir(mode=0o700, exist_ok=True)
    s.need(root.resolve() == root, 'Frontend release root alias refused')
    target = root / manifest['revision']
    if target.exists():
        s.need(inspect(target) == manifest, 'Revision already has different assets')
        return manifest
    import uuid
    pending = root / ('.pending-' + uuid.uuid4().hex)
    shutil.copytree(source, pending, symlinks=True)
    validate(pending, manifest)
    # Flush the complete immutable tree before publishing its directory name.
    import start_binary
    start_binary.sync_tree(pending)
    os.rename(pending, target)
    fd = os.open(root, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    return manifest


def verify_selection(instance, selection):
    manifest = selection['frontend']
    sha = revision(manifest['revision'])
    s.need(inspect(instance / 'frontend-releases' / sha) == manifest,
           'Selected frontend differs from retained artifact')


def health(manifest):
    base = PREFIX + revision(manifest['revision']) + '/'
    info = s.get('/api/info')['data'].get('build_info') or {}
    s.need(info.get('frontend_revision') == manifest['revision'], 'Served frontend revision differs')
    import hashlib
    for name, digest in manifest['files'].items():
        if name == 'index.html' or name.endswith(('.js', '.css')):
            url = '/' if name == 'index.html' else base + name
            s.need(hashlib.sha256(s.get_bytes(url)).hexdigest() == digest,
                   'Served frontend differs from selected artifact')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dist', required=True, type=Path)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    package(args.dist.absolute(), args.revision, args.output.absolute())
