"""Seed a completed conversation in the disposable instance, never start an agent."""
import datetime
import json
import sqlite3
import sys
import uuid

connection = sqlite3.connect(sys.argv[1])
connection.execute('PRAGMA foreign_keys = ON')
workspace_id = uuid.UUID(sys.argv[2])
root = sys.argv[3]
session, run, turn, attempt, request, correlation, event_id = [uuid.uuid4() for _ in range(7)]
now = datetime.datetime.now(datetime.timezone.utc).isoformat()


def insert(table, **values):
    columns = {row[1] for row in connection.execute(f'PRAGMA table_info({table})')}
    for column in ('created_at', 'updated_at'):
        if column in columns:
            values.setdefault(column, now)
    values = {k: v.bytes if isinstance(v, uuid.UUID) else json.dumps(v) if isinstance(v, dict) else v
              for k, v in values.items()}
    connection.execute(f'INSERT INTO {table} ({",".join(values)}) VALUES ({",".join("?" for _ in values)})',
                       list(values.values()))


# Interactive native layout, with an existing direct folder: no worktree creation.
connection.execute("UPDATE workspaces SET usage='interactive', workspace_kind='direct_folder', container_ownership='external' WHERE id=?", (workspace_id.bytes,))
insert('sessions', id=session, workspace_id=workspace_id, executor='CODEX', name='Native file link', agent_working_dir='project')
workspace = dict(workspace_id=str(workspace_id), mode='shared_workspace', path=root)
identity = dict(schema_version=1, payload_version=1, session_id=str(session), agent_run_id=str(run),
                turn_id=str(turn), correlation_id=str(correlation))
input_message = dict(message_id=str(uuid.uuid4()), role='user', content='Show the native file link.')
message = dict(message_id=str(uuid.uuid4()), role='assistant', content='[Open native source](src/example.ts:120)')
envelope = dict(identity, request_id=str(request), idempotency_key=str(request), intent='initial',
                runtime_profile_id='codex:default', provider_id='codex', workspace=workspace,
                input=input_message, created_at=now)
insert('agent_runs', id=run, session_id=session, workspace_id=workspace_id, request_id=request,
       idempotency_key=str(request), correlation_id=correlation, schema_version=1, payload_version=1,
       runtime_profile_id='codex:default', provider_id='codex', workspace_mode='shared_workspace',
       workspace_path=root, status='succeeded', request_envelope=envelope)
insert('agent_turns', id=turn, agent_run_id=run, request_id=request, intent='initial', input_message=input_message)
capabilities = dict(schema_version=1, runtime_profile_id='codex:default', provider_id='codex',
                    adapter_version='1', resolved_at=now, capabilities=[])
attempt_request = dict(identity, request_id=str(attempt), idempotency_key=str(attempt),
                       run_attempt_id=str(attempt), attempt_number=1, mode='launch', transport='app_server_jsonrpc',
                       runtime_profile_id='codex:default', provider_id='codex', workspace=workspace,
                       capability_snapshot=capabilities, executor_config=dict(executor='CODEX'), created_at=now)
insert('agent_run_attempts', id=attempt, agent_run_id=run, turn_id=turn, request_id=attempt,
       idempotency_key=str(attempt), attempt_number=1, mode='launch', transport='app_server_jsonrpc',
       schema_version=1, payload_version=1, capability_snapshot=capabilities,
       request_envelope=attempt_request, status='succeeded', started_at=now, finished_at=now)
event = dict(identity, event_id=str(event_id), run_attempt_id=str(attempt), run_attempt_number=1,
             sequence=1, timestamp=now, native_refs=[], payload=dict(type='message', data=dict(message=message, final_output=True)))
insert('agent_events', event_id=event_id, session_id=session, agent_run_id=run, turn_id=turn,
       run_attempt_id=attempt, run_attempt_number=1, sequence=1, correlation_id=correlation,
       schema_version=1, payload_version=1, event_envelope=event)
state = dict(state_schema_version=1, reducer_version=1, session_id=str(session), agent_run_id=str(run),
             turn_id=str(turn), status='succeeded', projection_status='current', last_run_attempt_id=str(attempt),
             last_run_attempt_number=1, last_event_sequence=1, last_event_id=str(event_id),
             terminal_output=message, unknown_event_count=0, updated_at=now)
insert('agent_run_state', agent_run_id=run, state_schema_version=1, reducer_version=1,
       last_run_attempt_id=attempt, last_run_attempt_number=1, last_event_sequence=1,
       last_event_id=event_id, status='succeeded', state_json=state)
connection.commit()
print(json.dumps(dict(session_id=str(session), run_id=str(run))))
