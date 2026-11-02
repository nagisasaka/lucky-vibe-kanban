import type { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';

export type ExecutionProcessRuntimeStatus = `${ExecutionProcessStatus}`;

export type ExecutionProcessRuntimeTone =
  | 'active'
  | 'danger'
  | 'neutral'
  | 'success'
  | 'warning';

export interface ExecutionProcessRuntimeDisplay {
  status: ExecutionProcessRuntimeStatus;
  label: string;
  tone: ExecutionProcessRuntimeTone;
  source: 'process_status';
  error: null;
  errorLabel: string | null;
}

const RUNTIME_DISPLAY: Record<
  ExecutionProcessRuntimeStatus,
  Pick<ExecutionProcessRuntimeDisplay, 'tone'>
> = {
  running: { tone: 'active' },
  completed: { tone: 'success' },
  failed: { tone: 'danger' },
  killed: { tone: 'neutral' },
};

export function getExecutionProcessLifecycle(
  process: ExecutionProcess
): ExecutionProcessRuntimeStatus {
  return process.status;
}

export function isExecutionProcessActive(process: ExecutionProcess): boolean {
  switch (getExecutionProcessLifecycle(process)) {
    case 'running':
      return true;
    case 'completed':
    case 'failed':
    case 'killed':
      return false;
  }
}

export function isChatBlockingScriptActive(process: ExecutionProcess): boolean {
  return (
    process.executor_action.typ.type === 'ScriptRequest' &&
    // Preview servers stay alive between chat turns and have their own stop control.
    process.run_reason !== 'devserver' &&
    isExecutionProcessActive(process)
  );
}

export function isExecutionProcessFailedLike(
  process: ExecutionProcess
): boolean {
  switch (getExecutionProcessLifecycle(process)) {
    case 'failed':
    case 'killed':
      return true;
    case 'running':
    case 'completed':
      return false;
  }
}

export function isExecutionProcessCompleted(
  process: ExecutionProcess
): boolean {
  return getExecutionProcessLifecycle(process) === 'completed';
}

export function getExecutionProcessRuntimeDisplay(
  process: ExecutionProcess
): ExecutionProcessRuntimeDisplay {
  const status = getExecutionProcessLifecycle(process);
  const display = RUNTIME_DISPLAY[status];

  return {
    status,
    label: status,
    tone: display.tone,
    source: 'process_status',
    error: null,
    errorLabel: null,
  };
}
