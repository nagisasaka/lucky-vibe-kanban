"""Add inert preview samples to a stopped, retained LOCAL-81 fixture instance."""
import sqlite3
import uuid
from pathlib import Path
from datetime import datetime, timezone

root = Path('.build/kanban-validation/retained-instance.txt').read_text().strip()
root = Path(root).resolve()
assert root.parent == Path('.build/kanban-validation').resolve()
assert root.name.startswith('instance-')
assert not (root / 'preview-seeded').exists(), 'fixture already seeded'
with sqlite3.connect(root / 'data/db.v2.sqlite') as db:
    db.execute('PRAGMA foreign_keys=ON')
    db.create_function('datetime', -1, lambda *args: datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S.%f'))
    assert db.execute('SELECT COUNT(*) FROM access_settings').fetchone()[0] == 0, 'do not overwrite configured authentication'
    project = db.execute('SELECT id FROM projects LIMIT 1').fetchone()[0]
    statuses = db.execute('SELECT id,name FROM local_project_statuses WHERE project_id=? ORDER BY sort_order', (project,)).fetchall()
    number = db.execute('SELECT COALESCE(MAX(issue_number),0) FROM local_issues WHERE project_id=?',(project,)).fetchone()[0]
    for position, ((status, name), count) in enumerate(zip(statuses, [0,1,4,12])):
        issue = uuid.uuid4().bytes
        number += 1
        db.execute('INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order) VALUES(?,?,?,?,?,?,?,?)',
            (issue,project,number,f'LOCAL-{number}',status,f'Preview: {name} / {count} Workspaces',
             '# 独立fixture\n本文は2行プレビュー。**詳細**では全文を参照できます。\nhttps://example.invalid/'+'long-url/'*50,-100+position))
        for n in range(count):
            workspace = uuid.uuid4().bytes
            db.execute('INSERT INTO workspaces(id,branch,name) VALUES(?,?,?)',(workspace,'fixture/no-agent',f'Preview Workspace {n+1} (no agent run)'))
            db.execute('INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)',(workspace,project,issue))
(root / 'preview-seeded').write_text('LOCAL-81 inert fixture: no AgentRuns/credentials, no production data\n')
print('Preview project:', str(uuid.UUID(bytes=project)))
