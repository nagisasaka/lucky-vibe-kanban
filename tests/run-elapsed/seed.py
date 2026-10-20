"""Synthetic canonical provider records in the test-owned SQLite database only."""
import datetime as dt
import json
import sqlite3
import sys
import uuid
from pathlib import Path

instance, project, issue, mode, *anchors = sys.argv[1:]
assert '/run-elapsed-validation/instance-' in str(Path(instance).resolve())
con = sqlite3.connect(Path(instance) / 'data/db.v2.sqlite')
con.execute('PRAGMA foreign_keys=ON')
uid = lambda: str(uuid.uuid4())
blob = lambda value: uuid.UUID(value).bytes
now = dt.datetime.now(dt.timezone.utc)
stamp = lambda value: value.isoformat().replace('+00:00', 'Z')
meta_path = Path(instance) / 'fixture.json'
def insert(table, values):
    columns = {row[1] for row in con.execute(f'PRAGMA table_info({table})')}
    # Python's SQLite may predate the subsec modifier used by server defaults.
    for key in ('created_at', 'updated_at'):
        if key in columns:
            values.setdefault(key, stamp(now))
    cols = ','.join(values)
    con.execute(f'INSERT INTO {table} ({cols}) VALUES ({",".join("?" for _ in values)})', list(values.values()))

def event(run, turn, attempt, sequence, time, payload, session):
    eid = uid()
    envelope = dict(schema_version=1, payload_version=1, event_id=eid, session_id=session,
                    agent_run_id=run, turn_id=turn, run_attempt_id=attempt, run_attempt_number=1,
                    sequence=sequence, correlation_id=run, timestamp=time, native_refs=[], payload=payload)
    insert('agent_events', dict(event_id=blob(eid), session_id=blob(session), agent_run_id=blob(run),
           turn_id=blob(turn), run_attempt_id=blob(attempt), run_attempt_number=1, sequence=sequence,
           schema_version=1, payload_version=1, correlation_id=blob(run), event_envelope=json.dumps(envelope)))
    return eid

if mode == 'seed':
    ws, session = anchors
    # Workspace, Git repository and session are created through the real API.
    insert('local_workspace_links', dict(workspace_id=blob(ws), project_id=blob(project), issue_id=blob(issue)))
    runs = []
    for index, status in enumerate(['succeeded', 'running']):
        run, turn, attempt, req = uid(), uid(), uid(), uid()
        start = stamp(now - dt.timedelta(seconds=120 if index == 0 else 60))
        end = stamp(now - dt.timedelta(seconds=108)) if index == 0 else None
        workspace = dict(workspace_id=ws, mode='shared_workspace', path=instance)
        request = dict(schema_version=1, payload_version=1, request_id=req, idempotency_key=run,
                       session_id=session, agent_run_id=run, turn_id=turn, correlation_id=run,
                       intent='initial', runtime_profile_id='CODEX:default', provider_id='codex',
                       workspace=workspace, input=dict(message_id=uid(), role='user', content='Elapsed test'), created_at=start)
        insert('agent_runs', dict(id=blob(run), session_id=blob(session), workspace_id=blob(ws), request_id=blob(req),
               idempotency_key=run, correlation_id=blob(run), schema_version=1, payload_version=1,
               runtime_profile_id='CODEX:default', provider_id='codex', workspace_mode='shared_workspace',
               workspace_path=instance, status=status, request_envelope=json.dumps(request), created_at=start))
        insert('agent_turns', dict(id=blob(turn), agent_run_id=blob(run), request_id=blob(req), intent='initial', input_message=json.dumps(request['input'])))
        capabilities = dict(schema_version=1, runtime_profile_id='CODEX:default', provider_id='codex',
                            adapter_version='fixture', resolved_at=start, capabilities=[])
        attempt_request = dict(request, request_id=uid(), idempotency_key=attempt, run_attempt_id=attempt,
                               attempt_number=1, mode='launch', transport='app_server_jsonrpc',
                               capability_snapshot=capabilities, executor_config=dict(executor='CODEX', variant='default'))
        insert('agent_run_attempts', dict(id=blob(attempt), agent_run_id=blob(run), turn_id=blob(turn), request_id=blob(attempt_request['request_id']),
               idempotency_key=attempt, attempt_number=1, mode='launch', transport='app_server_jsonrpc', schema_version=1,
               payload_version=1, capability_snapshot=json.dumps(capabilities), request_envelope=json.dumps(attempt_request), status=status, started_at=start, finished_at=end))
        event(run, turn, attempt, 1, start, dict(type='message', data=dict(message=request['input'], final_output=False)), session)
        eid = event(run, turn, attempt, 2, end or start, dict(type='message', data=dict(
            message=dict(message_id=uid(), role='assistant', content=f'Elapsed fixture {index}'), final_output=index == 0)), session)
        state = dict(state_schema_version=1, reducer_version=1, session_id=session, agent_run_id=run, turn_id=turn,
                     status=status, projection_status='current', last_run_attempt_id=attempt, last_run_attempt_number=1,
                     last_event_sequence=2, last_event_id=eid, unknown_event_count=0, updated_at=stamp(now))
        if index == 1:
            state['goal'] = dict(status='active', objective='Elapsed Goal fixture', tokens_used=0, time_used_seconds=7, token_budget=None)
        insert('agent_run_state', dict(agent_run_id=blob(run), state_schema_version=1, reducer_version=1,
               last_run_attempt_id=blob(attempt), last_run_attempt_number=1, last_event_sequence=2, last_event_id=blob(eid),
               status=status, state_json=json.dumps(state), updated_at=stamp(now)))
        runs.append(dict(run=run, turn=turn, attempt=attempt, start=start))
    meta = dict(workspace=ws, session=session, runs=runs)
    meta_path.write_text(json.dumps(meta))
else:
    meta = json.loads(meta_path.read_text())
    run = meta['runs'][-1]
    state = json.loads(con.execute('SELECT state_json FROM agent_run_state WHERE agent_run_id=?', (blob(run['run']),)).fetchone()[0])
    terminal = mode in ['succeeded', 'failed', 'cancelled', 'crashed', 'audit_failed']
    end = stamp(dt.datetime.fromisoformat(run['start'].replace('Z', '+00:00')) + dt.timedelta(seconds=42)) if terminal else None
    seq = state['last_event_sequence'] + 1
    eid = event(run['run'], run['turn'], run['attempt'], seq, end or stamp(now), dict(type='lifecycle_changed', data=dict(status=mode)), meta['session'])
    state.update(status=mode, last_event_sequence=seq, last_event_id=eid, updated_at=stamp(now))
    con.execute('UPDATE agent_run_state SET status=?,state_json=?,last_event_sequence=?,last_event_id=?,updated_at=? WHERE agent_run_id=?',
                (mode, json.dumps(state), seq, blob(eid), stamp(now), blob(run['run'])))
    con.execute('UPDATE agent_runs SET status=?,updated_at=? WHERE id=?', (mode, stamp(now), blob(run['run'])))
    con.execute('UPDATE agent_run_attempts SET status=?,finished_at=? WHERE id=?', (mode, end, blob(run['attempt'])))
con.commit()
con.close()
print(json.dumps(meta))
