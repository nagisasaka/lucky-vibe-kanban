import { create } from 'zustand';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';

export interface PendingChatMessage {
  id: string;
  scope: string;
  content: string;
  timestamp: string;
  state: 'sending' | 'accepted' | 'failed' | 'unconfirmed';
  runId?: string;
  error?: string;
  baseline: string[];
}
/** A transport error cannot prove the server rejected the submission. */
export function deliveryFailure(
  error: unknown
): Pick<PendingChatMessage, 'state' | 'error'> {
  const status =
    error && typeof error === 'object' && 'status' in error
      ? error.status
      : null;
  const rejected =
    typeof status === 'number' &&
    status >= 400 &&
    status < 500 &&
    status !== 408;
  return {
    state: rejected ? 'failed' : 'unconfirmed',
    error: error instanceof Error ? error.message : 'Unknown error',
  };
}
export const chatMessageScope = (
  host: string | null,
  workspace: string | undefined,
  session: string | undefined
) => JSON.stringify([host, workspace, session ?? 'new']);

export function matchesPendingMessage(
  message: PendingChatMessage,
  entry: PatchTypeWithKey
) {
  return (
    entry.type === 'NORMALIZED_ENTRY' &&
    entry.content.entry_type.type === 'user_message' &&
    !message.baseline.includes(entry.patchKey) &&
    entry.content.content === message.content &&
    (!message.runId || entry.canonical?.agentRunId === message.runId)
  );
}
export const usePendingChatMessages = create<{
  messages: PendingChatMessage[];
  begin: (
    scope: string,
    content: string,
    entries: PatchTypeWithKey[]
  ) => string;
  update: (id: string, update: Partial<PendingChatMessage>) => void;
  remove: (ids: string[]) => void;
  reconcile: (scope: string, entries: PatchTypeWithKey[]) => void;
}>((set) => ({
  messages: [],
  begin: (scope, content, entries) => {
    const id = crypto.randomUUID();
    set((s) => ({
      messages: [
        ...s.messages.filter(
          (m) =>
            !(
              m.scope === scope &&
              m.content === content &&
              m.state === 'failed'
            )
        ),
        {
          id,
          scope,
          content,
          timestamp: new Date().toISOString(),
          state: 'sending',
          baseline: entries.map((e) => e.patchKey),
        },
      ],
    }));
    return id;
  },
  update: (id, update) =>
    set((s) => ({
      messages: s.messages.map((m) => (m.id === id ? { ...m, ...update } : m)),
    })),
  reconcile: (scope, entries) =>
    set((s) => {
      const confirmed = new Set(
        confirmedPendingIds(
          entries,
          s.messages.filter((m) => m.scope === scope)
        )
      );
      if (!confirmed.size) return s;
      const observedUserKeys = entries
        .filter(
          (e) =>
            e.type === 'NORMALIZED_ENTRY' &&
            e.content.entry_type.type === 'user_message'
        )
        .map((e) => e.patchKey);
      return {
        messages: s.messages
          .filter((m) => !confirmed.has(m.id))
          .map((m) =>
            m.scope === scope
              ? {
                  ...m,
                  baseline: [...new Set([...m.baseline, ...observedUserKeys])],
                }
              : m
          ),
      };
    }),
  remove: (ids) =>
    set((s) => ({ messages: s.messages.filter((m) => !ids.includes(m.id)) })),
}));

export function confirmedPendingIds(
  entries: PatchTypeWithKey[],
  messages: PendingChatMessage[]
): string[] {
  const used = new Set<string>();
  return messages
    .filter((message) => {
      const match = entries.find(
        (entry) =>
          !used.has(entry.patchKey) && matchesPendingMessage(message, entry)
      );
      if (!match) return false;
      used.add(match.patchKey);
      return true;
    })
    .map((message) => message.id);
}

export function appendPendingMessages(
  entries: PatchTypeWithKey[],
  messages: PendingChatMessage[]
): PatchTypeWithKey[] {
  const confirmed = new Set(confirmedPendingIds(entries, messages));
  return [
    ...entries,
    ...messages
      .filter((m) => !confirmed.has(m.id))
      .map(
        (m): PatchTypeWithKey => ({
          type: 'NORMALIZED_ENTRY',
          patchKey: `pending-message:${m.id}`,
          content: {
            entry_type: { type: 'user_message' },
            content: m.content,
            timestamp: m.timestamp,
          },
          pendingDelivery: { state: m.state, error: m.error },
        })
      ),
  ];
}
