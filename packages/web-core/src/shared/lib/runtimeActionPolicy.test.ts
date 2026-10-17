import { describe, expect, it } from 'vitest';
import {
  AgentProviderCapability,
  AgentProviderReadiness,
  BaseCodingAgent,
  ExecutionProcessStatus,
  type AgentProviderPolicy,
  type ExecutionProcess,
} from 'shared/types';
import { isChatBlockingScriptActive } from './executionProcessRuntime';
import { deriveRuntimeActionPolicy } from './runtimeActionPolicy';

const readyPolicy: AgentProviderPolicy = {
  executor: BaseCodingAgent.CODEX,
  readiness: AgentProviderReadiness.READY,
  capabilities: [
    AgentProviderCapability.INITIAL_RUN,
    AgentProviderCapability.FOLLOW_UP,
    AgentProviderCapability.SESSION_RESUME,
  ],
  legacy: false,
  disabled: false,
  diagnostics: [],
};

function baseInput(
  overrides: Partial<Parameters<typeof deriveRuntimeActionPolicy>[0]> = {}
): Parameters<typeof deriveRuntimeActionPolicy>[0] {
  return {
    hasContent: true,
    hasWorkspace: true,
    hasSession: true,
    hasExecutor: true,
    isNewSessionMode: false,
    hasPriorAgentRun: false,
    isAgentRunActive: false,
    isAgentRunCancelling: false,
    isLatestAgentRunTerminal: false,
    isStandaloneScriptActive: false,
    canCancelAgentRun: false,
    canResolveApproval: false,
    canSubmitInput: false,
    isSending: false,
    isStopping: false,
    isQueueLoading: false,
    isQueued: false,
    hasPendingApproval: false,
    hasPendingQuestion: false,
    isApprovalTimedOut: false,
    providerPolicy: readyPolicy,
    ...overrides,
  };
}

describe('runtime action policy', () => {
  const devServer: ExecutionProcess = {
    id: 'dev-server',
    session_id: 'session-1',
    run_reason: 'devserver',
    executor_action: {
      typ: {
        type: 'ScriptRequest',
        script: 'pnpm dev',
        language: 'Bash',
        context: 'DevServer',
        working_dir: null,
      },
      next_action: null,
    },
    status: ExecutionProcessStatus.running,
    exit_code: null,
    dropped: false,
    started_at: '2026-10-17T00:00:00Z',
    completed_at: null,
    created_at: '2026-10-17T00:00:00Z',
    updated_at: '2026-10-17T00:00:00Z',
  };

  it('allows chat sends without offering chat stop for a running dev server', () => {
    const scriptState = {
      isStandaloneScriptActive: [devServer].some(isChatBlockingScriptActive),
    };
    const initial = deriveRuntimeActionPolicy(baseInput(scriptState));
    const followUp = deriveRuntimeActionPolicy(
      baseInput({
        ...scriptState,
        hasPriorAgentRun: true,
        isLatestAgentRunTerminal: true,
      })
    );

    expect(initial.send_initial.allowed).toBe(true);
    expect(followUp.send_follow_up.allowed).toBe(true);
    expect(followUp.queue_follow_up.allowed).toBe(false);
    expect(initial.stop.allowed).toBe(false);
    expect(followUp.stop.allowed).toBe(false);
  });

  it('still allows cancelling an agent while a dev server is running', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        isStandaloneScriptActive: [devServer].some(isChatBlockingScriptActive),
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
      })
    );

    expect(policy.send_follow_up.reason).toBe('runtime_busy');
    expect(policy.queue_follow_up.allowed).toBe(true);
    expect(policy.stop.allowed).toBe(true);
  });

  it('allows idle new session initial send', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasSession: false,
        isNewSessionMode: true,
      })
    );

    expect(policy.send_initial.allowed).toBe(true);
    expect(policy.send_follow_up.reason).toBe('no_session');
  });

  it('allows existing idle follow-up after a prior coding run', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isLatestAgentRunTerminal: true,
      })
    );

    expect(policy.send_follow_up.allowed).toBe(true);
    expect(policy.queue_follow_up.allowed).toBe(false);
  });

  it('blocks direct follow-up and allows queue while running', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
      })
    );

    expect(policy.send_follow_up.reason).toBe('runtime_busy');
    expect(policy.queue_follow_up.allowed).toBe(true);
    expect(policy.stop.allowed).toBe(true);
  });

  it('allows appending to a queue and cancelling it', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
        isQueued: true,
      })
    );

    expect(policy.queue_follow_up.allowed).toBe(true);
    expect(policy.cancel_queue.allowed).toBe(true);
  });

  it('blocks send and queue while stopping', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
        isStopping: true,
      })
    );

    expect(policy.send_follow_up.reason).toBe('runtime_cancelling');
    expect(policy.queue_follow_up.reason).toBe('runtime_cancelling');
    expect(policy.stop.reason).toBe('runtime_cancelling');
  });

  it('blocks sends and allows stop while a standalone script is active', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({ isStandaloneScriptActive: true })
    );

    expect(policy.send_follow_up.reason).toBe('unknown_runtime');
    expect(policy.send_initial.reason).toBe('runtime_busy');
    expect(policy.stop.allowed).toBe(true);
  });

  it('allows follow-up after a failed terminal run', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isLatestAgentRunTerminal: true,
      })
    );

    expect(policy.send_follow_up.allowed).toBe(true);
    expect(policy.queue_follow_up.reason).toBe('runtime_terminal');
  });

  it('preserves approval approve and request-change rules', () => {
    const approvePolicy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
        canResolveApproval: true,
        hasContent: false,
        hasPendingApproval: true,
      })
    );
    const requestChangesPolicy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
        canResolveApproval: true,
        hasPendingApproval: true,
      })
    );

    expect(approvePolicy.approve.allowed).toBe(true);
    expect(approvePolicy.request_changes.reason).toBe('no_content');
    expect(requestChangesPolicy.request_changes.allowed).toBe(true);
    expect(requestChangesPolicy.send_follow_up.reason).toBe(
      'approval_required'
    );
  });

  it('allows question answers while question mode owns input', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isAgentRunActive: true,
        canCancelAgentRun: true,
        canSubmitInput: true,
        hasPendingQuestion: true,
      })
    );

    expect(policy.answer_question.allowed).toBe(true);
    expect(policy.send_follow_up.reason).toBe('question_required');
  });

  it('uses provider capability when policy exists', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isLatestAgentRunTerminal: true,
        providerPolicy: {
          ...readyPolicy,
          capabilities: [AgentProviderCapability.INITIAL_RUN],
        },
      })
    );

    expect(policy.send_follow_up.reason).toBe('provider_capability_missing');
  });

  it('falls back compatibly when provider policy is absent', () => {
    const policy = deriveRuntimeActionPolicy(
      baseInput({
        hasPriorAgentRun: true,
        isLatestAgentRunTerminal: true,
        providerPolicy: null,
      })
    );

    expect(policy.send_follow_up.allowed).toBe(true);
  });
});
