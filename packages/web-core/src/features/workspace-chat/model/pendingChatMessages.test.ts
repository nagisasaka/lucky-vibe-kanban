import { beforeEach, expect, it } from 'vitest';
import {
  appendPendingMessages,
  deliveryFailure,
  chatMessageScope,
  confirmedPendingIds,
  usePendingChatMessages,
  type PendingChatMessage,
} from './pendingChatMessages';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';
const entry = (
  key: string,
  content = 'Hello',
  run = 'run'
): PatchTypeWithKey => ({
  type: 'NORMALIZED_ENTRY',
  patchKey: key,
  content: { entry_type: { type: 'user_message' }, content, timestamp: null },
  canonical: {
    agentRunId: run,
    runAttemptId: 'attempt',
    runAttemptNumber: 1,
    eventId: key,
    eventIds: [key],
    sequence: 1n,
    active: false,
  },
});
const pending = (
  extra: Partial<PendingChatMessage> = {}
): PendingChatMessage => ({
  id: 'pending',
  scope: 'scope',
  content: 'Hello',
  timestamp: '2026-10-25T14:00:00Z',
  state: 'sending',
  baseline: ['old'],
  ...extra,
});
beforeEach(() =>
  usePendingChatMessages.setState({ messages: [], identities: {} })
);
it('shows a local message before ACK, and retains it after ACK until the canonical event arrives', () => {
  const old = entry('old');
  const p = pending();
  expect(appendPendingMessages([old], [p])).toHaveLength(2);
  expect(
    appendPendingMessages(
      [old],
      [pending({ state: 'accepted', runId: 'new-run' })]
    ).at(-1)?.pendingDelivery?.state
  ).toBe('accepted');
  expect(
    appendPendingMessages(
      [old, entry('new', 'Hello', 'new-run')],
      [pending({ state: 'accepted', runId: 'new-run' })]
    )
  ).toHaveLength(2);
});
it('reconciles event-before-ACK without matching identical old text or the wrong run', () => {
  expect(confirmedPendingIds([entry('old')], [pending()])).toEqual([]);
  expect(confirmedPendingIds([entry('new')], [pending()])).toEqual(['pending']);
  expect(
    confirmedPendingIds(
      [entry('new', 'Hello', 'wrong')],
      [pending({ runId: 'right' })]
    )
  ).toEqual([]);
  expect(
    confirmedPendingIds(
      [entry('new', 'Different', 'right')],
      [pending({ runId: 'right' })]
    )
  ).toEqual([]);
});
it('matches one canonical message to at most one local send, even for repeated same-run steering', () => {
  expect(
    confirmedPendingIds(
      [entry('new')],
      [
        pending({ id: 'one', runId: 'run' }),
        pending({ id: 'two', runId: 'run' }),
      ]
    )
  ).toEqual(['one']);
  expect(
    appendPendingMessages(
      [entry('new')],
      [pending({ id: 'one' }), pending({ id: 'two' })]
    )
  ).toHaveLength(2);
});
it('keeps failed text explicit and replaces its local row on retry without modifying historical messages', () => {
  const store = usePendingChatMessages.getState();
  const id = store.begin('scope', 'Hello', [entry('old')]);
  store.update(id, { state: 'failed', error: '403 Forbidden' });
  expect(
    appendPendingMessages([], usePendingChatMessages.getState().messages)[0]
      .pendingDelivery
  ).toEqual({ state: 'failed', error: '403 Forbidden' });
  const retry = store.begin('scope', 'Hello', [entry('old')]);
  expect(retry).not.toBe(id);
  expect(usePendingChatMessages.getState().messages).toHaveLength(1);
  store.remove([retry]);
  store.update(retry, { state: 'accepted', runId: 'run' });
  expect(usePendingChatMessages.getState().messages).toEqual([]);
  expect(chatMessageScope('host-a', 'ws', 's')).not.toBe(
    chatMessageScope('host-b', 'ws', 's')
  );
});

it('retains one-to-one event consumption across repeated reconciliation renders', () => {
  const store = usePendingChatMessages.getState();
  const p1 = store.begin('scope', 'Hello', [entry('old')]);
  const p2 = store.begin('scope', 'Hello', [entry('old')]);
  store.update(p1, { state: 'accepted', runId: 'run' });
  store.update(p2, { state: 'accepted', runId: 'run' });
  store.reconcile('scope', [entry('old'), entry('first')]);
  expect(usePendingChatMessages.getState().messages.map((m) => m.id)).toEqual([
    p2,
  ]);
  store.reconcile('scope', [entry('old'), entry('first')]);
  expect(usePendingChatMessages.getState().messages.map((m) => m.id)).toEqual([
    p2,
  ]);
  expect(
    appendPendingMessages(
      [entry('old'), entry('first')],
      usePendingChatMessages.getState().messages
    )
  ).toHaveLength(3);
  store.reconcile('scope', [entry('old'), entry('first'), entry('second')]);
  expect(usePendingChatMessages.getState().messages).toEqual([]);
});
it('does not treat lost responses as rejection or discard their pending identity on retry', () => {
  expect(deliveryFailure(new TypeError('Failed to fetch')).state).toBe(
    'unconfirmed'
  );
  expect(
    deliveryFailure(Object.assign(new Error('Forbidden'), { status: 403 }))
      .state
  ).toBe('failed');
  expect(
    deliveryFailure(
      Object.assign(new Error('Gateway timeout'), { status: 504 })
    ).state
  ).toBe('unconfirmed');
  const store = usePendingChatMessages.getState();
  const id = store.begin('scope', 'Hello', []);
  store.update(id, deliveryFailure(new TypeError('Failed to fetch')));
  store.begin('scope', 'Hello', []);
  expect(usePendingChatMessages.getState().messages).toHaveLength(2);
});

it('retains row identity and canonical patch key across ACK and repeated replay', () => {
  const store = usePendingChatMessages.getState();
  const id = store.begin('scope', 'Hello', [entry('old')]);
  const local = appendPendingMessages(
    [],
    usePendingChatMessages.getState().messages
  )[0];
  store.update(id, { state: 'accepted', runId: 'run' });
  const canonical = entry('new');
  const matched = appendPendingMessages(
    [canonical],
    usePendingChatMessages.getState().messages
  )[0];
  expect(matched.displayIdentity).toBe(local.displayIdentity);
  expect(matched.patchKey).toBe('new');
  store.reconcile('scope', [canonical]);
  const state = usePendingChatMessages.getState();
  expect(state.messages).toHaveLength(0);
  expect(
    appendPendingMessages([canonical], [], state.identities.scope)[0]
      .displayIdentity
  ).toBe(local.displayIdentity);
});
