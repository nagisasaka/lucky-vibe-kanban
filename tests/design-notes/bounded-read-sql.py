"""Lightweight SQL smoke tests; NOT a substitute for Rust/API/browser tests.
Uses production SQL strings and the real migration with synthetic in-memory data.
"""
import json
from pathlib import Path
import re
import sqlite3
import uuid

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / 'crates/db/src/models/design_note_read.rs').read_text()
delete_source = (ROOT / 'crates/server/src/routes/local_remote.rs').read_text()


def statement(text, prefix):
    matches = re.findall(r'"(' + re.escape(prefix) + r'[^"\n]*)"', text)
    assert len(matches) == 1, (prefix, len(matches))
    return matches[0]


conn = sqlite3.connect(':memory:')
conn.row_factory = sqlite3.Row
conn.executescript('PRAGMA foreign_keys=ON; CREATE TABLE projects(id BLOB PRIMARY KEY, name TEXT DEFAULT "fixture"); CREATE TABLE local_project_metadata(project_id BLOB, organization_id BLOB);'
                   'CREATE TABLE children(id INTEGER PRIMARY KEY, project_id BLOB REFERENCES projects(id) ON DELETE CASCADE);')
conn.executescript((ROOT / 'crates/db/migrations/20261023000000_design_notes.sql').read_text())
p, empty = uuid.uuid4().bytes, uuid.uuid4().bytes
conn.executemany('INSERT INTO projects(id) VALUES(?)', [(p,), (empty,)])
conn.execute('INSERT INTO children VALUES(1,?)', (p,))
body = '理😀e\u0301\n' * 20000
ids = sorted(uuid.uuid4().bytes for _ in range(44))
for note in ids:
    conn.execute('INSERT INTO design_notes VALUES(?,?,?,?,?,?,?,?,?)',
                 (note, p, 'title', body, 1, 'human', None, 'created', 'updated'))
    conn.execute('INSERT INTO design_note_revisions VALUES(?,?,?)',
                 (note, 1, json.dumps({'body': body, 'revision': 1, 'updated_at': 'updated'}, ensure_ascii=False)))
notes_query = statement(source, 'SELECT id,title,revision,created_at,updated_at')
seen, cursor = [], None
while True:
    rows = conn.execute(notes_query, (p, cursor, cursor, 21)).fetchall()
    assert len(rows) <= 21
    assert all('body' not in r.keys() for r in rows)
    seen.extend(r['id'] for r in rows[:20])
    if len(rows) <= 20:
        break
    cursor = rows[19]['id']
assert seen == ids

note = ids[0]
text_query = statement(source, 'SELECT revision,length(snapshot)').format(table='design_note_revisions', key='note_id')
first = conn.execute(text_query, (1, 397, note, None, None)).fetchone()
conn.execute('UPDATE design_notes SET body=?,revision=2 WHERE id=?', ('changed', note))
conn.execute('INSERT INTO design_note_revisions VALUES(?,?,?)',
             (note, 2, json.dumps({'body': 'changed', 'revision': 2, 'updated_at': 'new'})))
parts, offset = [], 0
while True:
    row = conn.execute(text_query, (offset + 1, 397, note, first['revision'], first['revision'])).fetchone()
    assert row['revision'] == 1
    assert len(row['content']) <= 397
    parts.append(row['content'])
    offset += 397
    if offset >= row['total']:
        break
assert json.loads(''.join(parts))['body'] == body
assert conn.execute(text_query, (1, 397, note, None, None)).fetchone()['revision'] == 2
assert conn.execute(text_query, (1, 397, ids[1], 2, 2)).fetchone() is None

addition_ids = sorted(uuid.uuid4().bytes for _ in range(43))
for addition in addition_ids:
    conn.execute('INSERT INTO design_note_additions VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
                 (addition, note, 'heading', body, 'proposal', 'AI', 'test/source', 'speaker', 1, None, False, 1, 'human', 'created', 'updated'))
    for rev in (range(1, 45) if addition == addition_ids[0] else [1]):
        conn.execute('INSERT INTO design_note_addition_revisions VALUES(?,?,?)',
                     (addition, rev, json.dumps({'content': body, 'source': 'test/source', 'revision': rev, 'updated_at': 'updated'}, ensure_ascii=False)))
addition_query = statement(source, 'SELECT id,heading,kind,hidden,revision')
seen, cursor = [], None
while True:
    rows = conn.execute(addition_query, (note, cursor, cursor, 21)).fetchall()
    assert len(rows) <= 21
    assert all('content' not in r.keys() for r in rows)
    seen.extend(r['id'] for r in rows[:20])
    if len(rows) <= 20:
        break
    cursor = rows[19]['id']
assert seen == addition_ids
history_query = statement(source, 'SELECT revision,json_extract').format(table='design_note_addition_revisions', key='addition_id')
seen, after = [], 0
while True:
    rows = conn.execute(history_query, (addition_ids[0], after, 21)).fetchall()
    assert len(rows) <= 21
    seen.extend(r['revision'] for r in rows[:20])
    if len(rows) <= 20:
        break
    after = rows[19]['revision']
assert seen == list(range(1, 45))

# Upgrade a populated old schema, preserving every canonical SQL value.
tables = ['design_notes','design_note_revisions','design_note_additions','design_note_addition_revisions','design_note_requests']
def snapshot():
    return {t: [tuple(r) for r in conn.execute('SELECT * FROM '+t+' ORDER BY rowid')] for t in tables}
conn.execute('INSERT INTO design_note_requests VALUES(?,?,?,?,?)', (p, 'human', uuid.uuid4().bytes, '{}', '{}'))
before = snapshot()
conn.commit()
conn.executescript('BEGIN;'+(ROOT / 'crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql').read_text()+'COMMIT;')
assert snapshot() == before
delete_query = statement(delete_source, 'DELETE FROM projects WHERE id=?')
assert conn.execute(delete_query, (p,)).rowcount == 1
assert snapshot() == before
assert conn.execute('SELECT deleted_at FROM design_note_projects WHERE id=?',(p,)).fetchone()[0]
assert conn.execute('PRAGMA foreign_key_check').fetchall() == []
assert conn.execute(delete_query, (p,)).rowcount == 0
assert conn.execute(delete_query, (empty,)).rowcount == 1
assert conn.execute(delete_query, (empty,)).rowcount == 0
conn.close()
print('PASS: bounded metadata SQL, Unicode/pinned snapshot, history continuation, old-schema migration and retained DELETE with exact row equality')
