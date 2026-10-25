"""Seed Git summary evidence only in the isolated.mjs-owned disposable instance."""
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import uuid
from datetime import datetime, timezone

root = Path(sys.argv[1]).resolve()
assert root.parent == Path('.build/kanban-validation').resolve()
assert root.name.startswith('instance-')
ids = json.loads(sys.argv[2])
assert len(ids) == 2

def git(path, *args):
    return subprocess.check_output(['git', '-C', str(path), *args], stderr=subprocess.DEVNULL)

with sqlite3.connect(root / 'data/db.v2.sqlite') as db:
    db.execute('PRAGMA foreign_keys=ON')
    db.create_function('datetime', -1, lambda *args: datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S.%f'))
    for i, workspace_id in enumerate(ids):
        parent = root / 'fixture-worktrees' / workspace_id
        repo = parent / 'repo'
        repo.mkdir(parents=True)
        git(repo, 'init', '-b', 'main')
        git(repo, 'config', 'user.name', 'LOCAL81 Fixture')
        git(repo, 'config', 'user.email', 'fixture@example.invalid')
        (repo / 'base.txt').write_text('baseline\n')
        git(repo, 'add', '.')
        git(repo, 'commit', '-m', 'baseline')
        git(repo, 'checkout', '-b', 'feature')
        if i == 0:
            (repo / 'committed.txt').write_text('committed\n')
            git(repo, 'add', '.')
            git(repo, 'commit', '-m', 'feature')
            (repo / 'base.txt').write_text('changed\n')
            (repo / 'untracked.txt').write_text('untracked\n')
        repo_id = uuid.uuid4().bytes
        db.execute('INSERT INTO repos(id,path,name,display_name) VALUES(?,?,?,?)', (repo_id,str(repo),'repo',f'fixture-{i}'))
        db.execute('INSERT INTO workspace_repos(id,workspace_id,repo_id,target_branch) VALUES(?,?,?,?)', (uuid.uuid4().bytes,uuid.UUID(workspace_id).bytes,repo_id,'main'))
        db.execute("UPDATE workspaces SET container_ref=?,branch='feature' WHERE id=?", (str(parent),uuid.UUID(workspace_id).bytes))
print('Seeded two isolated Git workspaces: committed/uncommitted/untracked and clean')
