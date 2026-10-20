import { describe, expect, it } from 'vitest';
import {
  AgentRuntimeToolStatus,
  type AgentEventEnvelope,
  type AgentEventPayload,
  type AgentRunSummary,
  type RunState,
} from 'shared/types';
import { projectCanonicalAgentConversation } from './canonicalAgentConversation';
import { buildCanonicalAgentSessionTimeline } from './canonicalAgentSessionTimeline';
import {
  emptyCanonicalAgentTimeline,
  mergeCanonicalAgentTimeline,
  mergeAgentLiveEvent,
  clearAgentLiveEvents,
} from './canonicalAgentTimeline';

const SESSION_ID = 'session-1';

function state(runId: string, status: RunState['status']): RunState {
  return {
    state_schema_version: 1,
    reducer_version: 1,
    session_id: SESSION_ID,
    agent_run_id: runId,
    turn_id: `turn-${runId}`,
    status,
    projection_status: 'current',
    last_run_attempt_id: `attempt-${runId}`,
    last_run_attempt_number: 1,
    last_event_sequence: 0n,
    last_event_id: null,
    provider_session: null,
    terminal_output: null,
    last_error: null,
    unknown_event_count: 0n,
    updated_at: '2026-08-14T00:00:00Z',
  };
}

function event(
  runId: string,
  sequence: number,
  payload: AgentEventPayload
): AgentEventEnvelope {
  return {
    schema_version: 1,
    payload_version: 1,
    event_id: `${runId}-event-${sequence}`,
    session_id: SESSION_ID,
    agent_run_id: runId,
    turn_id: `turn-${runId}`,
    run_attempt_id: `attempt-${runId}`,
    run_attempt_number: 1,
    sequence: BigInt(sequence),
    correlation_id: `correlation-${runId}`,
    timestamp: `2026-08-14T00:00:${String(sequence).padStart(2, '0')}Z`,
    native_refs: [],
    payload,
  };
}

function projectRuns(
  runs: Array<{
    runId: string;
    status: RunState['status'];
    events: AgentEventEnvelope[];
    transientEvents?: AgentEventEnvelope[];
  }>
) {
  const summaries: AgentRunSummary[] = [];
  const timelines = new Map();

  runs.forEach((run, index) => {
    const runState = state(run.runId, run.status);
    summaries.push({
      agent_run_id: run.runId,
      session_id: SESSION_ID,
      turn_id: `turn-${run.runId}`,
      state: runState,
      created_at: `2026-08-14T00:0${index}:00Z`,
      updated_at: `2026-08-14T00:0${index}:00Z`,
    });
    timelines.set(run.runId, {
      ...mergeCanonicalAgentTimeline(
        emptyCanonicalAgentTimeline(),
        run.events,
        runState
      ),
      transientEvents: run.transientEvents ?? [],
    });
  });

  return projectCanonicalAgentConversation(
    buildCanonicalAgentSessionTimeline(SESSION_ID, summaries, timelines)
  );
}

describe('projectCanonicalAgentConversation', () => {
  it('keeps persisted steering between provider messages across replay', () => {
    const message = (
      sequence: number,
      role: 'user' | 'assistant',
      content: string
    ) =>
      event('steered', sequence, {
        type: 'message',
        data: {
          message: { message_id: `message-${sequence}`, role, content },
          final_output: false,
        },
      });
    const before = message(2, 'assistant', 'before');
    before.native_refs = [
      { sequence: 900n } as AgentEventEnvelope['native_refs'][number],
    ];
    const steering = message(3, 'user', 'steer');
    const after = message(4, 'assistant', 'after');
    after.native_refs = [
      { sequence: 1000n } as AgentEventEnvelope['native_refs'][number],
    ];
    const live = {
      ...after,
      event_id: 'live-after',
      sequence: 999n,
      native_refs: [],
    };
    const streaming = projectRuns([
      {
        runId: 'steered',
        status: 'running',
        events: [before, steering],
        transientEvents: [live],
      },
    ]);
    expect(
      streaming.entries
        .filter((entry) =>
          ['user_message', 'assistant_message'].includes(
            entry.content.entry_type.type
          )
        )
        .map((entry) => entry.content.content)
    ).toEqual(['before', 'steer', 'after']);
    for (const events of [
      [before, steering, after],
      [after, steering, before, steering],
    ]) {
      const projection = projectRuns([
        { runId: 'steered', status: 'succeeded', events },
      ]);
      expect(
        projection.entries
          .filter((entry) =>
            ['user_message', 'assistant_message'].includes(
              entry.content.entry_type.type
            )
          )
          .map((entry) => entry.content.content)
      ).toEqual(['before', 'steer', 'after']);
      expect(
        projection.entries.every(
          (entry) => entry.canonical?.agentRunId === 'steered'
        )
      ).toBe(true);
    }
  });

  it('shows a running shell command and ordered live output, then replaces it with the durable result', () => {
    const runId = 'shell-live';
    const runState = state(runId, 'running');
    const tool = (
      status: AgentRuntimeToolStatus,
      result: null | { aggregatedOutput: string; exitCode: number }
    ) => ({
      type: 'tool_call' as const,
      data: {
        tool_call_id: 'shell-1',
        tool_name: 'Shell',
        status,
        arguments: { command: 'printf first; sleep 1; printf second' },
        result,
      },
    });
    let timeline = mergeCanonicalAgentTimeline(
      emptyCanonicalAgentTimeline(),
      [event(runId, 1, tool(AgentRuntimeToolStatus.running, null))],
      runState
    );
    const project = () =>
      projectCanonicalAgentConversation(
        buildCanonicalAgentSessionTimeline(
          SESSION_ID,
          [
            {
              agent_run_id: runId,
              session_id: SESSION_ID,
              turn_id: `turn-${runId}`,
              state: runState,
              created_at: runState.updated_at,
              updated_at: runState.updated_at,
            },
          ],
          new Map([[runId, timeline]])
        )
      ).entries[0];
    expect(project()).toMatchObject({
      content: {
        entry_type: {
          action_type: {
            action: 'command_run',
            command: 'printf first; sleep 1; printf second',
            result: { output: null },
          },
        },
      },
    });
    const delta = (sequence: number, text: string) => ({
      schema_version: 1,
      event_id: `shell-live-${sequence}`,
      session_id: SESSION_ID,
      agent_run_id: runId,
      turn_id: `turn-${runId}`,
      run_attempt_id: `attempt-${runId}`,
      run_attempt_number: 1,
      native_sequence: sequence,
      timestamp: runState.updated_at,
      payload: {
        type: 'tool_output_delta' as const,
        data: { provider_item_id: 'shell-1', delta: text },
      },
    });
    const cursor = timeline.cursor;
    timeline = mergeAgentLiveEvent(timeline, delta(3, 'second'));
    timeline = mergeAgentLiveEvent(timeline, delta(2, 'first'));
    timeline = mergeAgentLiveEvent(timeline, delta(2, 'first'));
    expect(timeline.cursor).toEqual(cursor);
    expect(timeline.toolOutputDeltas).toHaveLength(2);
    expect(project()).toMatchObject({
      canonical: { active: true },
      content: {
        entry_type: {
          action_type: {
            result: { output: 'firstsecond' },
          },
        },
      },
    });
    expect(clearAgentLiveEvents(timeline).toolOutputDeltas).toEqual([]);
    timeline = mergeCanonicalAgentTimeline(timeline, [
      event(
        runId,
        4,
        tool(AgentRuntimeToolStatus.succeeded, {
          aggregatedOutput: 'firstsecond\n',
          exitCode: 0,
        })
      ),
    ]);
    expect(timeline.toolOutputDeltas).toEqual([]);
    expect(mergeAgentLiveEvent(timeline, delta(5, 'late'))).toBe(timeline);
    expect(project()).toMatchObject({
      canonical: { active: false },
      content: {
        entry_type: {
          action_type: {
            result: {
              output: 'firstsecond\n',
              exit_status: { type: 'exit_code', code: 0 },
            },
          },
        },
      },
    });
  });

  it('does not let a generic completion hide a failed child turn; a new turn can recover', () => {
    const child = (sequence: number, kind: string) =>
      event('r', sequence, {
        type: 'agent_activity',
        data: {
          activity: {
            thread_id: 'c',
            parent_thread_id: 'p',
            agent_path: '/root/c',
            kind,
            content: null,
          },
        },
      });
    const events = [
      child(1, 'running'),
      child(2, 'failed'),
      child(3, 'completed'),
    ];
    const status = (events: AgentEventEnvelope[]) => {
      const entry = projectRuns([{ runId: 'r', status: 'running', events }])
        .entries[0];
      if (
        entry.type !== 'NORMALIZED_ENTRY' ||
        entry.content.entry_type.type !== 'tool_use'
      )
        throw new Error('missing card');
      return entry.content.entry_type.status.status;
    };
    expect(status(events)).toBe('failed');
    expect(
      status([...events, child(4, 'running'), child(5, 'completed')])
    ).toBe('success');
  });

  it('shows only observed child lifecycle state and restores it from replay', () => {
    const child = (runId: string, sequence: number, kind: string) =>
      event(runId, sequence, {
        type: 'agent_activity',
        data: {
          activity: {
            thread_id: 'same-child',
            parent_thread_id: 'parent',
            agent_path: '/root/review',
            kind,
            content: kind === 'answer' ? 'Findings' : null,
          },
        },
      });
    const card = (kinds: string[], status: RunState['status'] = 'running') =>
      projectRuns([
        {
          runId: 'r',
          status,
          events: kinds.map((kind, index) => child('r', index + 1, kind)),
        },
      ]).entries[0];
    const expected = (label: string, active: boolean, status = 'created') => ({
      canonical: { active },
      content: {
        entry_type: {
          status: { status },
          action_type: { description: `parent → /root/review · ${label}` },
        },
      },
    });
    expect(card(['answer'])).toMatchObject(expected('unknown', false));
    expect(card(['pendingInit'])).toMatchObject(expected('initializing', true));
    expect(card(['running', 'shutdown'])).toMatchObject(
      expected('shutdown', false)
    );
    expect(card(['running', 'interrupt_requested'])).toMatchObject(
      expected('running', true)
    );
    expect(card(['running', 'tool', 'answer'])).toMatchObject(
      expected('running', true)
    );
    expect(card(['running', 'unknown'])).toMatchObject(
      expected('unknown', false)
    );
    expect(card(['completed', 'unknown'])).toMatchObject(
      expected('unknown', false)
    );
    expect(card(['completed', 'unknown', 'running'])).toMatchObject(
      expected('running', true)
    );
    expect(card(['completed', 'answer'])).toMatchObject(
      expected('completed', false, 'success')
    );
    expect(card(['failed', 'answer', 'completed'])).toMatchObject(
      expected('failed', false, 'failed')
    );
    expect(card(['interrupted', 'tool'])).toMatchObject(
      expected('interrupted', false, 'failed')
    );
    expect(card(['failed', 'running', 'answer'])).toMatchObject(
      expected('running', true)
    );
    expect(card(['running', 'answer'], 'succeeded')).toMatchObject(
      expected('unknown (last observed: running)', false)
    );

    // Reusing a native child thread in a later Run cannot overwrite history.
    const input = [
      {
        runId: 'old',
        status: 'succeeded' as const,
        events: [child('old', 1, 'running')],
      },
      {
        runId: 'new',
        status: 'running' as const,
        events: [child('new', 1, 'completed')],
      },
    ];
    const replay = projectRuns(input);
    expect(replay.entries[0]).toMatchObject(
      expected('unknown (last observed: running)', false)
    );
    expect(replay.entries[1]).toMatchObject(
      expected('completed', false, 'success')
    );
    expect(replay.entries[0].canonical?.agentRunId).toBe('old');
    expect(replay.entries[1].canonical?.agentRunId).toBe('new');
    expect(projectRuns(input)).toEqual(replay);
  });

  it('does not create child cards without child observations, including unloaded history', () => {
    const projection = projectRuns([
      {
        runId: 'r',
        status: 'running',
        events: [
          event('r', 1, {
            type: 'message',
            data: {
              message: {
                message_id: 'parent',
                role: 'assistant',
                content: 'Working',
              },
              final_output: false,
            },
          }),
        ],
      },
    ]);
    expect(
      projection.entries.some(
        (entry) =>
          entry.type === 'NORMALIZED_ENTRY' &&
          entry.content.entry_type.type === 'tool_use' &&
          entry.content.entry_type.action_type.action === 'task_create'
      )
    ).toBe(false);
    expect(
      projectRuns([{ runId: 'r', status: 'running', events: [] }]).entries
    ).toEqual([]);
  });

  it('keeps repeated parent live chunks together across child activity', () => {
    const runId = 'r';
    const runState = state(runId, 'running');
    let timeline = mergeCanonicalAgentTimeline(
      emptyCanonicalAgentTimeline(),
      [
        event(runId, 2, {
          type: 'agent_activity',
          data: {
            activity: {
              thread_id: 'child',
              parent_thread_id: 'parent',
              agent_path: null,
              kind: 'running',
              content: null,
            },
          },
        }),
      ],
      runState
    );
    for (const native_sequence of [1, 3]) {
      timeline = mergeAgentLiveEvent(timeline, {
        schema_version: 1,
        event_id: `live-${native_sequence}`,
        session_id: SESSION_ID,
        agent_run_id: runId,
        turn_id: `turn-${runId}`,
        run_attempt_id: `attempt-${runId}`,
        run_attempt_number: 1,
        native_sequence,
        timestamp: runState.updated_at,
        payload: {
          type: 'message_delta',
          data: {
            message_id: 'parent',
            provider_item_id: 'p',
            role: 'assistant',
            delta: 'ha',
          },
        },
      });
    }
    const projection = projectCanonicalAgentConversation(
      buildCanonicalAgentSessionTimeline(
        SESSION_ID,
        [
          {
            agent_run_id: runId,
            session_id: SESSION_ID,
            turn_id: `turn-${runId}`,
            state: runState,
            created_at: runState.updated_at,
            updated_at: runState.updated_at,
          },
        ],
        new Map([[runId, timeline]])
      )
    );
    const messages = projection.entries.filter(
      (e) =>
        e.type === 'NORMALIZED_ENTRY' &&
        e.content.entry_type.type === 'assistant_message'
    );
    expect(messages).toHaveLength(1);
    if (messages[0].type !== 'NORMALIZED_ENTRY')
      throw new Error('missing message');
    expect(messages[0].content.content).toBe('haha');
  });
  it('groups child answers and follow-ups separately from parent messages on replay', () => {
    const runId = 'agents';
    const child = (sequence: number, kind: string, content: string | null) =>
      event(runId, sequence, {
        type: 'agent_activity',
        data: {
          activity: {
            thread_id: 'child',
            parent_thread_id: 'parent',
            agent_path: '/root/research',
            kind,
            content,
          },
        },
      });
    const input = [
      {
        runId,
        status: 'succeeded' as const,
        events: [
          child(1, 'started', null),
          child(2, 'answer', 'Child findings'),
          child(3, 'completed', null),
          child(4, 'running', null),
          child(5, 'answer', 'Child re-review'),
          child(6, 'completed', null),
          event(runId, 7, {
            type: 'message',
            data: {
              message: {
                message_id: 'parent',
                role: 'assistant',
                content: 'Parent conclusion',
              },
              final_output: true,
            },
          }),
        ],
      },
    ];
    const projection = projectRuns(input);
    const entries = projection.entries.filter(
      (e) => e.type === 'NORMALIZED_ENTRY'
    );
    expect(entries).toHaveLength(2);
    const first = entries[0];
    expect(first.type).toBe('NORMALIZED_ENTRY');
    if (
      first.type !== 'NORMALIZED_ENTRY' ||
      first.content.entry_type.type !== 'tool_use'
    )
      throw new Error('missing child card');
    const action = first.content.entry_type.action_type;
    if (action.action !== 'task_create') throw new Error('wrong card');
    expect(action.description).toContain('parent → /root/research');
    expect(action.result?.value).toContain('Child findings');
    expect(action.result?.value).toContain('Child re-review');
    expect(first.content.entry_type.status.status).toBe('success');
    expect(projectRuns(input)).toEqual(projection);
  });
  it('projects canonical messages without inventing execution-process identity', () => {
    const runId = 'run-message';
    const projection = projectRuns([
      {
        runId,
        status: 'succeeded',
        events: [
          event(runId, 1, {
            type: 'message',
            data: {
              message: {
                message_id: 'user-1',
                role: 'user',
                content: 'Build it',
              },
              final_output: false,
            },
          }),
          event(runId, 2, {
            type: 'message',
            data: {
              message: {
                message_id: 'assistant-1',
                role: 'assistant',
                content: 'Hel',
              },
              final_output: false,
            },
          }),
          event(runId, 3, {
            type: 'message',
            data: {
              message: {
                message_id: 'assistant-1',
                role: 'assistant',
                content: 'Hello',
              },
              final_output: true,
            },
          }),
        ],
      },
    ]);

    expect(
      projection.entries.map((entry) => ({
        type:
          entry.type === 'NORMALIZED_ENTRY'
            ? entry.content.entry_type.type
            : entry.type,
        content: entry.type === 'NORMALIZED_ENTRY' ? entry.content.content : '',
      }))
    ).toEqual([
      { type: 'user_message', content: 'Build it' },
      { type: 'assistant_message', content: 'Hello' },
    ]);
    expect(
      projection.entries.every(
        (entry) => entry.executionProcessId === undefined
      )
    ).toBe(true);
  });

  it('aggregates tools by run and tool-call id and closes them at terminal state', () => {
    const firstRun = 'run-tool-1';
    const secondRun = 'run-tool-2';
    const projection = projectRuns([
      {
        runId: firstRun,
        status: 'succeeded',
        events: [
          event(firstRun, 1, {
            type: 'tool_call',
            data: {
              tool_call_id: 'shared-tool-id',
              tool_name: 'Shell',
              status: AgentRuntimeToolStatus.running,
              arguments: { command: 'pwd' },
              result: null,
            },
          }),
          event(firstRun, 2, {
            type: 'tool_call',
            data: {
              tool_call_id: 'shared-tool-id',
              tool_name: 'Shell',
              status: AgentRuntimeToolStatus.succeeded,
              arguments: { command: 'pwd' },
              result: '/workspace',
            },
          }),
        ],
      },
      {
        runId: secondRun,
        status: 'succeeded',
        events: [
          event(secondRun, 1, {
            type: 'tool_call',
            data: {
              tool_call_id: 'shared-tool-id',
              tool_name: 'Shell',
              status: AgentRuntimeToolStatus.running,
              arguments: { command: 'ls' },
              result: null,
            },
          }),
        ],
      },
    ]);
    const tools = projection.entries.filter(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'tool_use'
    );

    expect(tools).toHaveLength(2);
    expect(tools.map((entry) => entry.canonical?.agentRunId)).toEqual([
      firstRun,
      secondRun,
    ]);
    expect(tools[0]?.canonical?.eventIds).toHaveLength(2);
    expect(tools.every((entry) => entry.canonical?.active === false)).toBe(
      true
    );
  });

  it('resolves canonical approval and input controls in place', () => {
    const runId = 'run-control';
    const projection = projectRuns([
      {
        runId,
        status: 'succeeded',
        events: [
          event(runId, 1, {
            type: 'approval_requested',
            data: {
              approval_id: 'approval-1',
              tool_call_id: null,
              tool_name: 'Write',
            },
          }),
          event(runId, 2, {
            type: 'approval_resolved',
            data: {
              approval_id: 'approval-1',
              approved: false,
              reason: 'Not now',
            },
          }),
          event(runId, 3, {
            type: 'input_requested',
            data: { input_id: 'input-1', prompt: 'Which branch?' },
          }),
          event(runId, 4, {
            type: 'input_resolved',
            data: { input_id: 'input-1', answered: true },
          }),
        ],
      },
    ]);
    const controls = projection.entries.filter(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'tool_use'
    );

    expect(controls).toHaveLength(2);
    if (
      controls[0]?.type !== 'NORMALIZED_ENTRY' ||
      controls[0].content.entry_type.type !== 'tool_use' ||
      controls[1]?.type !== 'NORMALIZED_ENTRY' ||
      controls[1].content.entry_type.type !== 'tool_use'
    ) {
      throw new Error('Expected canonical control entries');
    }
    expect(controls[0].content.entry_type.status).toEqual({
      status: 'denied',
      reason: 'Not now',
    });
    expect(controls[0].canonical?.eventIds).toHaveLength(2);
    expect(controls[1].content.entry_type.status).toEqual({
      status: 'success',
    });
    expect(controls[1].canonical?.eventIds).toHaveLength(2);
  });

  it('keeps context separate from cumulative tokens and drops previous Run usage', () => {
    const usageEvent = event('old', 1, {
      type: 'token_usage',
      data: {
        input_tokens: 1000n,
        output_tokens: 200n,
        cached_input_tokens: 500n,
        context_tokens: 25n,
        model_context_window: 100n,
      },
    });
    const old = {
      runId: 'old',
      status: 'succeeded' as const,
      events: [usageEvent],
    };
    const projection = projectRuns([old]);
    const usage = projection.entries.find(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'token_usage_info'
    );
    expect(
      usage?.type === 'NORMALIZED_ENTRY' && usage.content.entry_type
    ).toMatchObject({
      total_tokens: 25,
      model_context_window: 100,
      cumulative_tokens: 1200,
    });
    const next = projectRuns([
      old,
      { runId: 'new', status: 'running', events: [] },
    ]);
    expect(
      next.entries.some(
        (entry) =>
          entry.type === 'NORMALIZED_ENTRY' &&
          entry.content.entry_type.type === 'token_usage_info'
      )
    ).toBe(false);
  });

  it('reports token totals without inventing a context window', () => {
    const runId = 'run-usage';
    const projection = projectRuns([
      {
        runId,
        status: 'succeeded',
        events: [
          event(runId, 1, {
            type: 'token_usage',
            data: {
              input_tokens: 12n,
              output_tokens: 5n,
              cached_input_tokens: 3n,
            },
          }),
        ],
      },
    ]);
    const usage = projection.entries.find(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'token_usage_info'
    );

    if (
      usage?.type !== 'NORMALIZED_ENTRY' ||
      usage.content.entry_type.type !== 'token_usage_info'
    ) {
      throw new Error('Expected token usage entry');
    }
    expect(usage.content.entry_type.total_tokens).toBe(0);
    expect(usage.content.entry_type.cumulative_tokens).toBe(17);
    expect(usage.content.entry_type.model_context_window).toBe(0);
  });
});
