import React from 'react';
import { createRoot } from 'react-dom/client';
import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import { AggregatedGroupEntry } from '../../packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry';
import { projectCanonicalAgentConversation } from '../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation';
import { buildCanonicalAgentSessionTimeline } from '../../packages/web-core/src/features/agent-runtime/model/canonicalAgentSessionTimeline';
import {
  emptyCanonicalAgentTimeline,
  mergeCanonicalAgentTimeline,
} from '../../packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline';
import { aggregateConsecutiveEntries } from '../../packages/web-core/src/shared/lib/aggregateEntries';
import { ChatRunFooter } from '../../packages/ui/src/components/ChatRunFooter';
import jaTasks from '../../packages/web-core/src/i18n/locales/ja/tasks.json';
import jaCommon from '../../packages/web-core/src/i18n/locales/ja/common.json';
import type { AgentEventEnvelope, AgentRunSummary } from '../../shared/types';
const stamp = '2026-10-25T10:54:00.000Z';
const summary: AgentRunSummary = {
  agent_run_id: 'run',
  session_id: 'session',
  turn_id: 'turn',
  created_at: stamp,
  updated_at: '2026-10-25T10:54:13Z',
  completed_at: '2026-10-25T10:54:13Z',
  state: {
    state_schema_version: 1,
    reducer_version: 1,
    agent_run_id: 'run',
    session_id: 'session',
    turn_id: 'turn',
    status: 'succeeded',
    projection_status: 'current',
    last_run_attempt_id: 'attempt',
    last_run_attempt_number: 1,
    last_event_sequence: 2n,
    unknown_event_count: 0n,
    updated_at: '2026-10-25T10:54:13Z',
  },
};
const events: AgentEventEnvelope[] = [
  {
    type: 'tool_call',
    data: {
      tool_call_id: 'cmd',
      tool_name: 'Shell',
      status: 'running',
      arguments: {
        command: "/bin/bash -lc 'ls -la'",
        cwd: '/fixture',
        source: 'agent',
        commandActions: [],
      },
    },
  },
  {
    type: 'tool_call',
    data: {
      tool_call_id: 'cmd',
      tool_name: 'Shell',
      status: 'succeeded',
      arguments: {
        command: "/bin/bash -lc 'ls -la'",
        cwd: '/fixture',
        source: 'agent',
        commandActions: [],
      },
      result: { aggregatedOutput: 'README.md\n', exitCode: 0, durationMs: 0 },
    },
  },
].map(
  (payload, index) =>
    ({
      schema_version: 1,
      payload_version: 1,
      event_id: `event-${index}`,
      session_id: 'session',
      agent_run_id: 'run',
      turn_id: 'turn',
      run_attempt_id: 'attempt',
      run_attempt_number: 1,
      sequence: BigInt(index + 1),
      correlation_id: 'correlation',
      timestamp: index === 0 ? stamp : '2026-10-25T10:54:00.001558Z',
      native_refs: [],
      payload,
    }) as AgentEventEnvelope
);
const projection = projectCanonicalAgentConversation(
  buildCanonicalAgentSessionTimeline(
    'session',
    [summary],
    new Map([
      [
        'run',
        mergeCanonicalAgentTimeline(
          emptyCanonicalAgentTimeline(),
          events,
          summary.state
        ),
      ],
    ])
  )
);
const rows = aggregateConsecutiveEntries(projection.entries);
(window as any).projectedRows = rows;
void i18next
  .use(initReactI18next)
  .init({
    lng: 'ja',
    fallbackLng: 'ja',
    interpolation: { escapeValue: false },
    resources: { ja: { tasks: jaTasks, common: jaCommon } },
  })
  .then(() =>
    createRoot(document.getElementById('root')!).render(
      <main className="bg-primary p-base text-normal">
        {rows.map((entry) =>
          entry.type === 'AGGREGATED_GROUP' ? (
            <section key={entry.patchKey} id="actual-group">
              <AggregatedGroupEntry group={entry} executionProcess={null} />
            </section>
          ) : 'runTiming' in entry && entry.runTiming ? (
            <section key={entry.patchKey} id="actual-footer">
              <ChatRunFooter {...entry.runTiming} />
            </section>
          ) : null
        )}
      </main>
    )
  );
