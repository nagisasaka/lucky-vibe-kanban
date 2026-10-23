"""Read only a caller-supplied isolated fixture; output no note content or secrets."""
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import uuid

path = Path(sys.argv[1]).resolve()
# This helper must never inspect a live instance database.
fixture_root = Path('.build/design-notes/retention-e2e').resolve()
if not path.is_relative_to(fixture_root) or not path.parent.parent.name.startswith('instance-'):
    raise ValueError('Only task-owned isolated E2E fixture databases are allowed')
conn = sqlite3.connect(path.as_uri() + '?mode=ro', uri=True)
conn.execute('BEGIN')
tables = {}
for table in ['design_notes', 'design_note_revisions', 'design_note_additions',
              'design_note_addition_revisions', 'design_note_requests']:
    fields = ','.join('quote("' + r[1] + '")' for r in conn.execute('PRAGMA table_info(' + table + ')'))
    rows = conn.execute('SELECT ' + fields + ' FROM ' + table + ' ORDER BY rowid').fetchall()
    tables[table] = {'rows': len(rows), 'sha256': hashlib.sha256(json.dumps(rows, ensure_ascii=False).encode()).hexdigest()}
project = uuid.UUID(sys.argv[2]).bytes
identity = conn.execute('SELECT hex(id),name,hex(organization_id),deleted_at FROM design_note_projects WHERE id=?', (project,)).fetchone()
print(json.dumps({'tables': tables,
 'project_exists': bool(conn.execute('SELECT 1 FROM projects WHERE id=?',(project,)).fetchone()),
 'identity': dict(zip(['id','name','organization_id','deleted_at'], identity)) if identity else None,
 'foreign_key_check': conn.execute('PRAGMA foreign_key_check').fetchall()}, ensure_ascii=False))
conn.close()
