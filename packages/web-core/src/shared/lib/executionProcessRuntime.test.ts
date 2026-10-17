import { describe, expect, it } from 'vitest';
import { ExecutionProcessStatus, type ExecutionProcess } from 'shared/types';
import {
  getExecutionProcessLifecycle,
  getExecutionProcessRuntimeDisplay,
  isChatBlockingScriptActive,
  isExecutionProcessActive,
  isExecutionProcessFailedLike,
} from './executionProcessRuntime';

const baseProcess = {
  id: 'process-1',
  session_id: 'session-1',
  run_reason: 'setupscript',
  executor_action: {
    typ: {
      type: 'ScriptRequest',
      script: 'echo test',
      language: 'Bash',
      context: 'SetupScript',
      working_dir: null,
    },
    next_action: null,
  },
  status: ExecutionProcessStatus.running,
  exit_code: null,
  dropped: false,
  started_at: '2026-06-22T00:00:00Z',
  completed_at: null,
  created_at: '2026-06-22T00:00:00Z',
  updated_at: '2026-06-22T00:00:00Z',
} as ExecutionProcess;

function makeProcess(overrides: Partial<ExecutionProcess>): ExecutionProcess {
  return {
    ...baseProcess,
    ...overrides,
  };
}

describe('execution process runtime helpers', () => {
  it('keeps a running dev server active without blocking chat', () => {
    const process = makeProcess({ run_reason: 'devserver' });

    expect(isExecutionProcessActive(process)).toBe(true);
    expect(isChatBlockingScriptActive(process)).toBe(false);
  });

  it.each<ExecutionProcess['run_reason']>([
    'setupscript',
    'cleanupscript',
    'archivescript',
    'integrationvalidation',
    'resourcecommand',
  ])('still blocks chat for an active %s', (run_reason) => {
    expect(isChatBlockingScriptActive(makeProcess({ run_reason }))).toBe(true);
  });

  it.each([
    ExecutionProcessStatus.completed,
    ExecutionProcessStatus.failed,
    ExecutionProcessStatus.killed,
  ])('does not block chat for a %s script', (status) => {
    expect(isChatBlockingScriptActive(makeProcess({ status }))).toBe(false);
  });

  it('uses process status for active state', () => {
    const process = makeProcess({
      status: ExecutionProcessStatus.running,
    });

    const display = getExecutionProcessRuntimeDisplay(process);

    expect(getExecutionProcessLifecycle(process)).toBe('running');
    expect(isExecutionProcessActive(process)).toBe(true);
    expect(display).toMatchObject({
      label: 'running',
      source: 'process_status',
      tone: 'active',
    });
  });

  it('treats failed process status as failed-like', () => {
    const process = makeProcess({
      status: ExecutionProcessStatus.failed,
    });

    expect(getExecutionProcessLifecycle(process)).toBe('failed');
    expect(isExecutionProcessActive(process)).toBe(false);
    expect(isExecutionProcessFailedLike(process)).toBe(true);
  });

  it('keeps killed fallback distinct from crashed', () => {
    const process = makeProcess({
      status: ExecutionProcessStatus.killed,
    });

    expect(getExecutionProcessLifecycle(process)).toBe('killed');
    expect(isExecutionProcessFailedLike(process)).toBe(true);
    expect(getExecutionProcessRuntimeDisplay(process)).toMatchObject({
      label: 'killed',
      tone: 'neutral',
    });
  });
});
