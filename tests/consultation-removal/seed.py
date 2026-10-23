"""Synthetic canonical provider records in the test-owned SQLite database only."""
import datetime as dt
import json
import sqlite3
import sys
import uuid
from pathlib import Path

instance, project, issue, mode, *anchors = sys.argv[1:]
assert '/consultation-removal/instance-' in str(Path(instance).resolve())
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

if mode in ['awaiting_input', 'awaiting_approval', 'succeeded']:
    ws, session = anchors
    # Workspace, Git repository and session are created through the real API.
    insert('local_workspace_links', dict(workspace_id=blob(ws), project_id=blob(project), issue_id=blob(issue)))
    runs = []
    for index, status in enumerate(['succeeded', mode]):
        run, turn, attempt, req = uid(), uid(), uid(), uid()
        start = stamp(now - dt.timedelta(seconds=120 if index == 0 else 60))
        end = stamp(now - dt.timedelta(seconds=108 if index == 0 else 48)) if status == 'succeeded' else None
        workspace = dict(workspace_id=ws, mode='shared_workspace', path=instance)
        request = dict(schema_version=1, payload_version=1, request_id=req, idempotency_key=run,
                       session_id=session, agent_run_id=run, turn_id=turn, correlation_id=run,
                       intent='initial', runtime_profile_id='CODEX:default', provider_id='codex',
                       workspace=workspace, input=dict(message_id=uid(), role='user', content='Consultation removal fixture'), created_at=start)
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
            message=dict(message_id=uid(), role='assistant', content=f'Consultation fixture {index}'), final_output=status == 'succeeded')), session)
        sequence = 2
        if index == 1 and mode != 'succeeded':
            sequence = 3
            payload = (dict(type='input_requested', data=dict(input_id='fixture-input', prompt='Which branch should we use?'))
                       if mode == 'awaiting_input' else
                       dict(type='approval_requested', data=dict(approval_id='fixture-approval', tool_name='Shell', tool_call_id=None)))
            eid = event(run, turn, attempt, sequence, start, payload, session)
        state = dict(state_schema_version=1, reducer_version=1, session_id=session, agent_run_id=run, turn_id=turn,
                     status=status, projection_status='current', last_run_attempt_id=attempt, last_run_attempt_number=1,
                     last_event_sequence=sequence, last_event_id=eid, unknown_event_count=0, updated_at=stamp(now))
        insert('agent_run_state', dict(agent_run_id=blob(run), state_schema_version=1, reducer_version=1,
               last_run_attempt_id=blob(attempt), last_run_attempt_number=1, last_event_sequence=sequence, last_event_id=blob(eid),
               status=status, state_json=json.dumps(state), updated_at=stamp(now)))
        runs.append(dict(run=run, turn=turn, attempt=attempt, start=start))
    meta = dict(workspace=ws, session=session, runs=runs)
    meta_path.write_text(json.dumps(meta))
con.commit()
con.close()
print(json.dumps(meta))
