import type {
  AgentEventEnvelope,
  ActionType,
  JsonValue,
  NormalizedEntry,
  PatchType,
  ToolStatus,
} from 'shared/types';
import { AgentRuntimeToolStatus } from 'shared/types';
import { newestRunState } from './canonicalAgentTimeline';

import type {
  CanonicalConversationIdentity,
  CanonicalConversationProjection,
  PatchTypeWithKey,
} from '@/shared/hooks/useConversationHistory/types';
import {
  isCanonicalRunActive,
  isCanonicalRunTerminal,
  type CanonicalAgentSessionRun,
  type CanonicalAgentSessionTimeline,
} from './canonicalAgentSessionTimeline';

function numericTokenCount(value: number | bigint): number {
  const count = typeof value === 'bigint' ? Number(value) : value;
  return Number.isSafeInteger(count) && count >= 0 ? count : NaN;
}

function displaySequence(event: AgentEventEnvelope): bigint {
  const nativeSequence = event.native_refs?.at(-1)?.sequence;
  const sequence = nativeSequence ?? event.sequence;
  return typeof sequence === 'bigint' ? sequence : BigInt(sequence);
}

function conversationEvents(
  run: CanonicalAgentSessionRun
): AgentEventEnvelope[] {
  if (!run.timeline) return [];
  // Only canonical sequence orders persisted events. Native audit counters can
  // be much larger and host-authored steering has no native reference at all.
  const durable = [...run.timeline.events].sort((a, b) => {
    if (a.run_attempt_number !== b.run_attempt_number)
      return a.run_attempt_number - b.run_attempt_number;
    return BigInt(a.sequence) < BigInt(b.sequence)
      ? -1
      : BigInt(a.sequence) > BigInt(b.sequence)
        ? 1
        : 0;
  });
  const positioned = durable.map((event, index) => ({
    event,
    position: index * 2,
  }));
  for (const event of run.timeline.transientEvents) {
    // Place live deltas after the last durable predecessor without comparing
    // canonical and native counters, or disturbing durable replay order.
    let position = -1;
    durable.forEach((candidate, index) => {
      const precedes =
        candidate.run_attempt_number < event.run_attempt_number ||
        (candidate.run_attempt_number === event.run_attempt_number &&
          ((candidate.native_refs?.length ?? 0) > 0
            ? displaySequence(candidate) <= displaySequence(event)
            : Date.parse(candidate.timestamp) <= Date.parse(event.timestamp)));
      if (precedes) position = index * 2 + 1;
    });
    positioned.push({ event, position });
  }
  return positioned
    .sort(
      (a, b) =>
        a.position - b.position ||
        (displaySequence(a.event) < displaySequence(b.event)
          ? -1
          : displaySequence(a.event) > displaySequence(b.event)
            ? 1
            : 0)
    )
    .map(({ event }) => event);
}

function identityFromEvents(
  events: readonly AgentEventEnvelope[],
  active: boolean
): CanonicalConversationIdentity {
  const first = events[0];
  const latest = events.at(-1) ?? first;
  return {
    agentRunId: latest.agent_run_id,
    runAttemptId: latest.run_attempt_id,
    runAttemptNumber: latest.run_attempt_number,
    eventId: latest.event_id,
    eventIds: events.map((event) => event.event_id),
    sequence:
      typeof latest.sequence === 'bigint'
        ? latest.sequence
        : BigInt(latest.sequence),
    active,
  };
}

function normalizedPatch(
  event: AgentEventEnvelope,
  entry: NormalizedEntry,
  active = false
): PatchTypeWithKey {
  return {
    type: 'NORMALIZED_ENTRY',
    content: entry,
    patchKey: `agent-event:${event.event_id}`,
    canonical: identityFromEvents([event], active),
  };
}

function toolStatus(
  status: AgentRuntimeToolStatus,
  approvalId?: string
): ToolStatus {
  switch (status) {
    case AgentRuntimeToolStatus.created:
    case AgentRuntimeToolStatus.running:
      return { status: 'created' };
    case AgentRuntimeToolStatus.approved:
      return { status: 'approved' };
    case AgentRuntimeToolStatus.waiting_approval:
      return approvalId
        ? { status: 'pending_approval', approval_id: approvalId }
        : { status: 'waiting_approval' };
    case AgentRuntimeToolStatus.denied:
      return { status: 'denied', reason: null };
    case AgentRuntimeToolStatus.cancelled:
      return { status: 'cancelled' };
    case AgentRuntimeToolStatus.unknown:
      return { status: 'unknown' };
    case AgentRuntimeToolStatus.succeeded:
      return { status: 'success' };
    case AgentRuntimeToolStatus.failed:
      return { status: 'failed' };
    case AgentRuntimeToolStatus.timed_out:
      return { status: 'timed_out' };
  }
}

function toolIsActive(status: AgentRuntimeToolStatus, runActive: boolean) {
  return (
    runActive &&
    (status === AgentRuntimeToolStatus.created ||
      status === AgentRuntimeToolStatus.running ||
      status === AgentRuntimeToolStatus.waiting_approval)
  );
}

function jsonObject(value: JsonValue | null | undefined) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value
    : null;
}

function toolEntry(
  event: AgentEventEnvelope,
  status: AgentRuntimeToolStatus,
  toolName: string,
  runActive: boolean,
  approvalId?: string,
  argumentsValue: Extract<
    AgentEventEnvelope['payload'],
    { type: 'tool_call' }
  >['data']['arguments'] = null,
  resultValue: Extract<
    AgentEventEnvelope['payload'],
    { type: 'tool_call' }
  >['data']['result'] = null
): PatchTypeWithKey {
  const result =
    resultValue == null
      ? null
      : { type: { type: 'json' as const }, value: resultValue };
  const args = jsonObject(argumentsValue);
  const shellResult = jsonObject(resultValue);
  const action: ActionType =
    (toolName === 'Shell' || toolName === 'Bash') &&
    typeof args?.command === 'string'
      ? {
          action: 'command_run',
          command: args.command,
          category: 'other',
          result: {
            output:
              typeof resultValue === 'string'
                ? resultValue
                : typeof shellResult?.aggregatedOutput === 'string'
                  ? shellResult.aggregatedOutput
                  : toolName === 'Bash' && resultValue != null
                    ? JSON.stringify(resultValue, null, 2)
                    : null,
            exit_status:
              typeof shellResult?.exitCode === 'number'
                ? { type: 'exit_code', code: shellResult.exitCode }
                : null,
          },
        }
      : {
          action: 'tool',
          tool_name: toolName,
          arguments: argumentsValue ?? null,
          result,
        };
  const patch = normalizedPatch(
    event,
    {
      entry_type: {
        type: 'tool_use',
        tool_name: toolName,
        action_type: action,
        status: toolStatus(status, approvalId),
      },
      content:
        resultValue == null
          ? toolName
          : typeof resultValue === 'string'
            ? resultValue
            : JSON.stringify(resultValue, null, 2),
      timestamp: event.timestamp,
    },
    toolIsActive(status, runActive)
  );
  const observedStart =
    status === AgentRuntimeToolStatus.created ||
    status === AgentRuntimeToolStatus.running;
  const observedEnd = [
    AgentRuntimeToolStatus.succeeded,
    AgentRuntimeToolStatus.failed,
    AgentRuntimeToolStatus.denied,
    AgentRuntimeToolStatus.timed_out,
    AgentRuntimeToolStatus.cancelled,
  ].includes(status);
  const duration = shellResult?.durationMs;
  patch.toolTiming = {
    startedAt: observedStart ? event.timestamp : null,
    endedAt: observedEnd ? event.timestamp : null,
    durationMs:
      typeof duration === 'number' && Number.isFinite(duration) && duration >= 0
        ? duration
        : null,
  };
  const hasOutput =
    action.action === 'command_run'
      ? action.result?.output != null
      : resultValue != null;
  patch.toolOutput = {
    input:
      argumentsValue == null ? null : JSON.stringify(argumentsValue, null, 2),
    state: hasOutput
      ? observedEnd
        ? 'complete'
        : 'partial'
      : observedEnd || !runActive
        ? 'unavailable'
        : 'pending',
    exitCode:
      typeof shellResult?.exitCode === 'number' ? shellResult.exitCode : null,
    cwd: typeof args?.cwd === 'string' ? args.cwd : null,
    language:
      resultValue !== null &&
      typeof resultValue === 'object' &&
      (action.action === 'tool' || toolName === 'Bash')
        ? 'json'
        : null,
  };
  return patch;
}

function mergeEventIdentity(
  entry: PatchTypeWithKey,
  event: AgentEventEnvelope,
  active: boolean
) {
  const previousIds = entry.canonical?.eventIds ?? [];
  entry.canonical = identityFromEvents(
    [
      ...previousIds.map(
        (eventId) => ({ ...event, event_id: eventId }) as AgentEventEnvelope
      ),
      event,
    ],
    active
  );
}

function appendRunEntries(
  run: CanonicalAgentSessionRun,
  output: PatchTypeWithKey[]
) {
  const liveIds = new Set(
    run.timeline?.transientEvents.map((event) => event.event_id)
  );
  const events = conversationEvents(run);
  const state = run.timeline?.state ?? run.summary.state;
  const runActive = isCanonicalRunActive(state);

  const toolIndexes = new Map<string, number>();
  // Result frames may omit the name and arguments supplied at tool start.
  // IDs are provider-local and may be reused by a later attempt.
  const toolDetails = new Map<
    string,
    { name: string; arguments: JsonValue | null }
  >();
  const toolKey = (event: AgentEventEnvelope, id: string) =>
    JSON.stringify([event.run_attempt_id, id]);
  const agentEntries = new Map<
    string,
    {
      index: number;
      content: string[];
      path: string;
      parent: string | null;
      status: ToolStatus;
      observedState: string;
    }
  >();
  const approvalIndexes = new Map<string, number>();
  const explanationIndexes = new Map<string, number>();
  const explanationParts = new Map<string, Map<number, string>>();
  const inputIndexes = new Map<string, number>();
  let previousMessage:
    | { index: number; messageId: string; role: string }
    | undefined;

  const resetMessageMerge = () => {
    previousMessage = undefined;
  };

  for (const event of events) {
    const payload = event.payload;
    switch (payload.type) {
      case 'agent_activity': {
        // Child observations can interleave with a parent's message deltas.
        // They belong to a separate card, not a new parent message boundary.
        const activity = payload.data.activity;
        let agent = agentEntries.get(activity.thread_id);
        if (!agent) {
          agent = {
            index: output.length,
            content: [],
            path: activity.agent_path ?? activity.thread_id,
            parent: activity.parent_thread_id,
            status: { status: 'created' },
            observedState: 'unknown',
          };
          agentEntries.set(activity.thread_id, agent);
          output.push(
            normalizedPatch(event, {
              entry_type: { type: 'system_message' },
              content: '',
              timestamp: event.timestamp,
            })
          );
        }
        if (activity.agent_path) agent.path = activity.agent_path;
        if (activity.parent_thread_id) agent.parent = activity.parent_thread_id;
        if (activity.content)
          agent.content.push(`### ${activity.kind}\n\n${activity.content}`);
        // Messages and tools describe activity, not a lifecycle transition.
        // Keep the last explicit state until another lifecycle observation arrives.
        if (activity.kind === 'completed' && agent.status.status !== 'failed') {
          agent.status = { status: 'success' };
          agent.observedState = 'completed';
        } else if (['failed', 'interrupted'].includes(activity.kind)) {
          agent.status = { status: 'failed' };
          agent.observedState = activity.kind;
        } else if (
          ['started', 'running', 'pendingInit', 'shutdown', 'unknown'].includes(
            activity.kind
          )
        ) {
          agent.status = { status: 'created' };
          agent.observedState =
            activity.kind === 'pendingInit' ? 'initializing' : activity.kind;
        }
        const status = agent.status;
        const observedActive = ['started', 'running', 'initializing'].includes(
          agent.observedState
        );
        const active = runActive && observedActive;
        // A parent ending is not evidence that the child completed. Historical
        // observations remain useful, but must not look like a live child run.
        const stateLabel =
          !runActive && observedActive
            ? `unknown (last observed: ${agent.observedState})`
            : agent.observedState;
        const entry = normalizedPatch(
          event,
          {
            entry_type: {
              type: 'tool_use',
              tool_name: 'Agent',
              action_type: {
                action: 'task_create',
                description: `${agent.parent ?? '?'} → ${agent.path} · ${stateLabel}`,
                subagent_type: agent.path,
                result: {
                  type: { type: 'markdown' },
                  value: agent.content.join('\n\n'),
                },
              },
              status,
            },
            content: agent.path,
            timestamp: event.timestamp,
          },
          active
        );
        entry.patchKey = output[agent.index].patchKey;
        entry.canonical = output[agent.index].canonical;
        mergeEventIdentity(entry, event, active);
        output[agent.index] = entry;
        break;
      }
      case 'message': {
        const message = payload.data.message;
        const entryType =
          message.role === 'user'
            ? 'user_message'
            : message.role === 'assistant'
              ? 'assistant_message'
              : 'system_message';
        const canMerge =
          message.role === 'assistant' &&
          previousMessage?.messageId === message.message_id &&
          previousMessage.role === message.role;
        if (canMerge && previousMessage) {
          const previous = output[previousMessage.index];
          if (previous.type === 'NORMALIZED_ENTRY') {
            const previousContent = previous.content.content;
            previous.content = {
              ...previous.content,
              content:
                !liveIds.has(event.event_id) &&
                message.content.startsWith(previousContent)
                  ? message.content
                  : `${previousContent}${message.content}`,
              timestamp: event.timestamp,
            };
            mergeEventIdentity(previous, event, runActive);
          }
          break;
        }
        output.push(
          normalizedPatch(event, {
            entry_type: { type: entryType },
            content: message.content,
            timestamp: event.timestamp,
          })
        );
        previousMessage = {
          index: output.length - 1,
          messageId: message.message_id,
          role: message.role,
        };
        break;
      }
      case 'thinking':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: { type: 'thinking' },
            content: payload.data.content,
            timestamp: event.timestamp,
          })
        );
        break;
      case 'tool_call': {
        resetMessageMerge();
        const toolId = payload.data.tool_call_id ?? event.event_id;
        const key = toolKey(event, toolId);
        const existingIndex = toolIndexes.get(key);
        const previousDetails = toolDetails.get(key);
        const details = {
          name:
            payload.data.tool_name === 'unknown_tool' && previousDetails
              ? previousDetails.name
              : payload.data.tool_name,
          arguments:
            payload.data.arguments ?? previousDetails?.arguments ?? null,
        };
        toolDetails.set(key, details);
        const next = toolEntry(
          event,
          payload.data.status,
          details.name,
          runActive,
          undefined,
          details.arguments,
          payload.data.result ??
            (() => {
              const deltas = (run.timeline?.toolOutputDeltas ?? [])
                .filter(
                  (live) =>
                    live.run_attempt_id === event.run_attempt_id &&
                    live.payload.type === 'tool_output_delta' &&
                    live.payload.data.provider_item_id === toolId
                )
                .sort((a, b) =>
                  Number(BigInt(a.native_sequence) - BigInt(b.native_sequence))
                );
              return deltas.length
                ? {
                    aggregatedOutput: deltas
                      .map((live) =>
                        live.payload.type === 'tool_output_delta'
                          ? live.payload.data.delta
                          : ''
                      )
                      .join(''),
                  }
                : null;
            })()
        );
        if (existingIndex === undefined) {
          output.push(next);
          toolIndexes.set(key, output.length - 1);
        } else {
          const previous = output[existingIndex];
          if (next.toolTiming) {
            next.toolTiming.startedAt =
              previous.toolTiming?.startedAt ?? next.toolTiming.startedAt;
          }
          next.patchKey = previous.patchKey;
          next.canonical = identityFromEvents(
            [
              ...(previous.canonical?.eventIds ?? []).map(
                (eventId) =>
                  ({ ...event, event_id: eventId }) as AgentEventEnvelope
              ),
              event,
            ],
            next.canonical?.active ?? false
          );
          output[existingIndex] = next;
        }
        break;
      }
      case 'approval_requested': {
        resetMessageMerge();
        const existingIndex = payload.data.tool_call_id
          ? toolIndexes.get(toolKey(event, payload.data.tool_call_id))
          : undefined;
        const next = toolEntry(
          event,
          AgentRuntimeToolStatus.waiting_approval,
          payload.data.tool_name,
          runActive,
          payload.data.approval_id
        );
        if (existingIndex === undefined) {
          output.push(next);
          if (payload.data.tool_call_id) {
            const key = toolKey(event, payload.data.tool_call_id);
            toolIndexes.set(key, output.length - 1);
            toolDetails.set(key, {
              name: payload.data.tool_name,
              arguments: null,
            });
          }
          approvalIndexes.set(
            toolKey(event, payload.data.approval_id),
            output.length - 1
          );
        } else {
          const previous = output[existingIndex];
          next.patchKey = previous.patchKey;
          next.toolTiming = previous.toolTiming;
          next.toolOutput = previous.toolOutput;
          if (
            previous.type === 'NORMALIZED_ENTRY' &&
            previous.content.entry_type.type === 'tool_use' &&
            next.type === 'NORMALIZED_ENTRY' &&
            next.content.entry_type.type === 'tool_use'
          ) {
            const action = previous.content.entry_type.action_type;
            const details =
              action.action === 'command_run'
                ? { command: action.command }
                : action.action === 'tool'
                  ? action.arguments
                  : null;
            next.content = {
              ...previous.content,
              content:
                details == null
                  ? previous.content.content
                  : '```json\n' + JSON.stringify(details, null, 2) + '\n```',
              entry_type: {
                ...previous.content.entry_type,
                status: next.content.entry_type.status,
              },
              timestamp: event.timestamp,
            };
          }
          mergeEventIdentity(previous, event, runActive);
          next.canonical = previous.canonical;
          output[existingIndex] = next;
          approvalIndexes.set(
            toolKey(event, payload.data.approval_id),
            existingIndex
          );
        }
        break;
      }
      case 'approval_resolved': {
        resetMessageMerge();
        const index = approvalIndexes.get(
          toolKey(event, payload.data.approval_id)
        );
        if (index !== undefined) {
          const previous = output[index];
          if (
            previous.type === 'NORMALIZED_ENTRY' &&
            previous.content.entry_type.type === 'tool_use'
          ) {
            previous.content = {
              ...previous.content,
              entry_type: {
                ...previous.content.entry_type,
                status: payload.data.approved
                  ? { status: 'approved' }
                  : { status: 'denied', reason: payload.data.reason ?? null },
              },
              timestamp: event.timestamp,
            };
            if (!payload.data.approved && previous.toolTiming) {
              previous.toolTiming.endedAt = event.timestamp;
            }
            if (
              !payload.data.approved &&
              previous.toolOutput?.state === 'pending'
            ) {
              previous.toolOutput = {
                ...previous.toolOutput,
                state: 'unavailable',
              };
            }
            mergeEventIdentity(previous, event, false);
          }
        }
        break;
      }
      case 'input_requested': {
        resetMessageMerge();
        output.push(
          normalizedPatch(
            event,
            {
              entry_type: {
                type: 'tool_use',
                tool_name: 'Agent input',
                action_type: {
                  action: 'ask_user_question',
                  questions: [
                    {
                      question: payload.data.prompt,
                      header: 'Agent input',
                      options: [],
                      multiSelect: false,
                    },
                  ],
                },
                status: {
                  status: 'pending_approval',
                  approval_id: payload.data.input_id,
                },
              },
              content: payload.data.prompt,
              timestamp: event.timestamp,
            },
            runActive
          )
        );
        inputIndexes.set(payload.data.input_id, output.length - 1);
        break;
      }
      case 'input_resolved': {
        resetMessageMerge();
        const index = inputIndexes.get(payload.data.input_id);
        if (index !== undefined) {
          const previous = output[index];
          if (
            previous.type === 'NORMALIZED_ENTRY' &&
            previous.content.entry_type.type === 'tool_use'
          ) {
            previous.content = {
              ...previous.content,
              entry_type: {
                ...previous.content.entry_type,
                status: payload.data.answered
                  ? { status: 'success' }
                  : { status: 'denied', reason: null },
              },
              timestamp: event.timestamp,
            };
            mergeEventIdentity(previous, event, false);
          }
        }
        break;
      }
      case 'token_usage':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: {
              type: 'token_usage_info',
              total_tokens:
                payload.data.context_tokens == null
                  ? 0
                  : numericTokenCount(payload.data.context_tokens),
              model_context_window:
                payload.data.context_tokens == null ||
                payload.data.model_context_window == null
                  ? 0
                  : numericTokenCount(payload.data.model_context_window),
              cumulative_tokens:
                numericTokenCount(payload.data.input_tokens) +
                numericTokenCount(payload.data.output_tokens),
            },
            content: '',
            timestamp: event.timestamp,
          })
        );
        break;
      case 'error':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: {
              type: 'error_message',
              error_type: { type: 'other' },
            },
            content: payload.data.error.message,
            timestamp: event.timestamp,
          })
        );
        break;
      case 'projection_degraded':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: { type: 'system_message' },
            content: payload.data.reason,
            timestamp: event.timestamp,
          })
        );
        break;
      case 'session_observed':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: { type: 'system_message' },
            content: `Agent session ${payload.data.provider_session.provider_session_id}`,
            timestamp: event.timestamp,
          })
        );
        break;
      case 'unknown':
        resetMessageMerge();
        output.push(
          normalizedPatch(event, {
            entry_type: { type: 'system_message' },
            content: `Unsupported agent event: ${payload.data.event_type}`,
            timestamp: event.timestamp,
          })
        );
        break;
      case 'provider_extension':
        resetMessageMerge();
        if (
          ['codex', 'claude_code', 'lvk_live'].includes(
            payload.data.provider_namespace
          ) &&
          [
            'plan',
            'reasoning_summary',
            'thinking',
            'reasoning_summary_delta',
          ].includes(payload.data.provider_event)
        ) {
          const data = jsonObject(payload.data.payload);
          if (typeof data?.content !== 'string' || !data.content.trim()) break;
          const partial =
            payload.data.provider_event === 'reasoning_summary_delta';
          const itemId =
            typeof data.provider_item_id === 'string'
              ? data.provider_item_id
              : null;
          const key = toolKey(event, itemId ?? event.event_id);
          const existingIndex = explanationIndexes.get(key);
          const previous =
            existingIndex === undefined ? undefined : output[existingIndex];
          let content = data.content;
          if (partial) {
            const partIndex =
              typeof data.summary_index === 'number' &&
              Number.isInteger(data.summary_index) &&
              data.summary_index >= 0
                ? data.summary_index
                : 0;
            const parts =
              explanationParts.get(key) ?? new Map<number, string>();
            parts.set(partIndex, (parts.get(partIndex) ?? '') + content);
            explanationParts.set(key, parts);
            content = [...parts]
              .sort(([a], [b]) => a - b)
              .map(([, text]) => text)
              .join('\n\n');
          }
          const next = normalizedPatch(event, {
            entry_type: { type: 'thinking' },
            content,
            timestamp: event.timestamp,
          });
          next.providerText = {
            kind:
              payload.data.provider_event === 'plan'
                ? 'plan'
                : payload.data.provider_event === 'thinking'
                  ? 'thinking'
                  : 'summary',
            provider:
              payload.data.provider_namespace === 'lvk_live'
                ? null
                : payload.data.provider_namespace,
            itemId,
            partial,
          };
          if (existingIndex === undefined) {
            explanationIndexes.set(key, output.length);
            output.push(next);
          } else {
            next.patchKey = previous!.patchKey;
            output[existingIndex] = next;
          }
        }
        break;
      case 'lifecycle_changed':
        resetMessageMerge();
        break;
      case 'goal_updated':
      case 'goal_cleared':
        // Goal progress has its own UI and must not split a streaming message.
        break;
    }
  }

  const hasPendingControl = output.some(
    (entry) =>
      entry.canonical?.agentRunId === run.summary.agent_run_id &&
      entry.type === 'NORMALIZED_ENTRY' &&
      entry.content.entry_type.type === 'tool_use' &&
      ['pending_approval', 'waiting_approval', 'approved'].includes(
        entry.content.entry_type.status.status
      )
  );
  if (runActive && !hasPendingControl) {
    const latestEvent = events.at(-1);
    if (latestEvent) {
      const loadingPatch: PatchType = {
        type: 'NORMALIZED_ENTRY',
        content: {
          entry_type: { type: 'loading' },
          content: '',
          timestamp: latestEvent.timestamp,
        },
      };
      output.push({
        ...loadingPatch,
        patchKey: `agent-run:${run.summary.agent_run_id}:loading`,
        canonical: identityFromEvents([latestEvent], true),
      });
    }
  }
  // One footer per canonical run. Derive from persisted timestamps on every
  // projection; reconnects and React remounts must never accumulate local time.
  const timingState = newestRunState(
    run.summary.state,
    run.timeline?.state ?? null
  )!;
  const terminal = isCanonicalRunTerminal(timingState);
  const terminalEvent = events.find(
    (event) =>
      event.run_attempt_id === timingState.last_run_attempt_id &&
      event.payload.type === 'lifecycle_changed' &&
      isCanonicalRunTerminal({ ...state, status: event.payload.data.status })
  );
  const completedAt =
    timingState.last_run_attempt_id === run.summary.state.last_run_attempt_id
      ? run.summary.completed_at
      : null;
  output.push({
    type: 'NORMALIZED_ENTRY',
    content: {
      entry_type: { type: 'system_message' },
      content: '',
      timestamp: run.summary.created_at,
    },
    patchKey: `agent-run:${run.summary.agent_run_id}:elapsed`,
    runTiming: {
      startedAt: run.summary.created_at,
      endedAt: terminal
        ? (completedAt ?? terminalEvent?.timestamp ?? null)
        : null,
      active: !terminal,
    },
  });
}

export function projectCanonicalAgentConversation(
  timeline: CanonicalAgentSessionTimeline | null,
  isLoading = false
): CanonicalConversationProjection {
  if (!timeline) {
    return {
      entries: [],
      activeAgentRunIds: new Set(),
      runCount: 0,
      isLoading,
      isRunning: false,
      projectionDegraded: false,
      latestStatus: null,
    };
  }

  const entries: PatchTypeWithKey[] = [];
  for (const run of timeline.runs) appendRunEntries(run, entries);

  // A new Run/attempt without usage must not inherit the previous context gauge.
  const latest = timeline.latestRun;
  const latestState = latest?.timeline?.state ?? latest?.summary.state;
  const currentEntries = entries.filter(
    (entry) =>
      entry.type !== 'NORMALIZED_ENTRY' ||
      entry.content.entry_type.type !== 'token_usage_info' ||
      (entry.canonical?.agentRunId === latest?.summary.agent_run_id &&
        (!latestState?.last_run_attempt_id ||
          entry.canonical?.runAttemptId === latestState.last_run_attempt_id))
  );

  return {
    entries: currentEntries,
    activeAgentRunIds: new Set(
      timeline.runs
        .filter((run) =>
          isCanonicalRunActive(run.timeline?.state ?? run.summary.state)
        )
        .map((run) => run.summary.agent_run_id)
    ),
    runCount: timeline.runs.length,
    isLoading,
    isRunning: timeline.isRunning,
    projectionDegraded: timeline.projectionDegraded,
    latestStatus:
      timeline.latestRun?.timeline?.state?.status ??
      timeline.latestRun?.summary.state.status ??
      null,
  };
}
