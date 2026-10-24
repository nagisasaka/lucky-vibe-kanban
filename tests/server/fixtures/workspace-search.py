"""Seed only a runner-owned disposable database; never accept a live DB path."""
import json
from pathlib import Path
import sqlite3
import sys
import uuid

root = Path(sys.argv[1]).resolve()
assert root.parent == Path('.build').resolve() and root.name.startswith('search-instance-')
assert (root / 'search-fixture-owner').read_text().startswith('LOCAL-162')

def uid(number):
    return uuid.UUID(int=number).bytes

with sqlite3.connect(root / 'data/db.v2.sqlite') as db:
    db.execute('PRAGMA foreign_keys=ON')
    # The host Python SQLite predates datetime('now', 'subsec'), used by the
    # production schema. Provide the same timestamp function for fixture writes.
    from datetime import datetime, timezone
    db.create_function('datetime', -1, lambda *args: datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S.%f'))
    if sys.argv[2] == 'update':
        db.execute("UPDATE local_issues SET simple_id='LOCAL-108', issue_number=108 WHERE id=?", (uid(1001),))
    else:
        assert sys.argv[2] == 'create'
        for n, name in [(101, 'Project One'), (102, 'Project Two')]:
            db.execute('INSERT INTO projects(id,name) VALUES(?,?)', (uid(n), name))
            db.execute('INSERT INTO local_project_metadata(project_id,organization_id) VALUES(?,?)', (uid(n), uid(2)))
            db.execute("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',1)", (uid(n+100), uid(n)))
        for n, project, number in [(1001,101,106),(1002,102,106),(1003,101,1060),(1004,101,107)]:
            db.execute('INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,?,?,?,?,1)', (uid(n),uid(project),number,f'LOCAL-{number}',uid(project+100),'Example issue'))
        fixtures = [
            (2001,'Alpha target',1001,101,False),
            (2002,'Beta sibling',1001,101,False),
            (2003,'Gamma other project',1002,102,False),
            (2004,'Delta longer',1003,101,False),
            (2005,'Epsilon different',1004,101,False),
            (2006,'Archive target',1001,101,True),
            (2007,'Legacy standalone',None,None,False),
        ]
        fixtures += [(3000+n,f'Pad {n:03}',None,None,False) for n in range(55)]
        for n,name,issue,project,archived in fixtures:
            db.execute("INSERT INTO workspaces(id,branch,name,archived,pinned,created_at) VALUES(?,?,?,?,?,?)", (uid(n),'feature/solo' if issue is None else 'feature/example',name,archived,n>=3000,'2026-01-01 00:00:00'))
            if issue:
                db.execute('INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)', (uid(n),uid(project),uid(issue)))
print(json.dumps({'project':str(uuid.UUID(int=101)), 'otherUser':str(uuid.UUID(int=999))}))
