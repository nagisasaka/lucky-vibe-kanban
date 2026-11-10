import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import type {
  StartPresetRequest,
  PresetDefaults,
  WorkflowTemplateListResponse,
  WorkflowTemplateResponse,
  CreateWorkflowRequest,
  UpdateWorkflowRequest,
  TriggerWorkflowRequest,
  CreateWorkflowAttemptRequest,
  DraftWorkspaceRepo,
  RunWorkflowAttemptRequest,
  WorkflowAttemptListResponse,
  WorkflowAttemptResponse,
  WorkflowRunResponse,
  WorkflowActionResponse,
} from 'shared/types';

const LOCAL_BASE = '/api/local/v1';

interface MutationResponse<T> {
  data: T;
  txid: number;
}

export type CreateWorkflowAttemptPayload = CreateWorkflowAttemptRequest & {
  repos?: DraftWorkspaceRepo[];
};

export type RunWorkflowAttemptPayload = RunWorkflowAttemptRequest & {
  repos?: DraftWorkspaceRepo[];
};

async function localFetch(
  path: string,
  init: RequestInit = {}
): Promise<Response> {
  const headers = new Headers(init.headers ?? {});
  if (init.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  return makeLocalApiRequest(`${LOCAL_BASE}${path}`, {
    ...init,
    headers,
    hostScope: 'none',
  });
}

async function parseError(
  response: Response,
  fallback: string
): Promise<Error> {
  try {
    const body = await response.json();
    const message = body?.message || body?.error || fallback;
    return Object.assign(
      new Error(`${message} (${response.status} ${response.statusText})`),
      { status: response.status }
    );
  } catch {
    return Object.assign(
      new Error(`${fallback} (${response.status} ${response.statusText})`),
      { status: response.status }
    );
  }
}

async function getJson<T>(
  response: Response,
  errorMessage: string
): Promise<T> {
  if (!response.ok) {
    throw await parseError(response, errorMessage);
  }
  return response.json();
}

async function mutate<T>(response: Response, errorMessage: string): Promise<T> {
  if (!response.ok) {
    throw await parseError(response, errorMessage);
  }
  const wrapped: MutationResponse<T> = await response.json();
  return wrapped.data;
}

export interface ApproveNodeRequest {
  message?: string;
}

export interface RejectNodeRequest {
  message?: string;
}

export interface SelectArenaWinnerRequest {
  workspace_id: string;
}

export interface SelectConditionBranchRequest {
  selected_target_node_ids: string[];
  reason?: string | null;
}

export const workflowApi = {
  async list(projectId: string): Promise<WorkflowTemplateListResponse> {
    return getJson(
      await localFetch(`/projects/${projectId}/workflows`),
      'Failed to list workflows'
    );
  },

  async create(
    projectId: string,
    payload: CreateWorkflowRequest
  ): Promise<WorkflowTemplateResponse> {
    return mutate(
      await localFetch(`/projects/${projectId}/workflows`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to create workflow'
    );
  },

  async get(workflowId: string): Promise<WorkflowTemplateResponse> {
    return getJson(
      await localFetch(`/workflows/${workflowId}`),
      'Failed to get workflow'
    );
  },

  async update(
    workflowId: string,
    payload: UpdateWorkflowRequest
  ): Promise<WorkflowTemplateResponse> {
    return mutate(
      await localFetch(`/workflows/${workflowId}`, {
        method: 'PUT',
        body: JSON.stringify(payload),
      }),
      'Failed to update workflow'
    );
  },

  async delete(workflowId: string): Promise<void> {
    const response = await localFetch(`/workflows/${workflowId}`, {
      method: 'DELETE',
    });
    if (!response.ok) {
      throw await parseError(response, 'Failed to delete workflow');
    }
    // Consume DeleteResponse payload
    await response.json();
  },

  async trigger(
    workflowId: string,
    payload: TriggerWorkflowRequest
  ): Promise<WorkflowRunResponse> {
    return mutate(
      await localFetch(`/workflows/${workflowId}/trigger`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to trigger workflow'
    );
  },

  async listAttempts(
    projectId: string,
    issueId: string
  ): Promise<WorkflowAttemptListResponse> {
    return getJson(
      await localFetch(
        `/projects/${projectId}/issues/${issueId}/workflow-attempts`
      ),
      'Failed to list workflow attempts'
    );
  },

  async listProjectAttempts(
    projectId: string
  ): Promise<WorkflowAttemptListResponse> {
    return getJson(
      await localFetch(`/projects/${projectId}/workflow-attempts`),
      'Failed to list project workflow attempts'
    );
  },

  async createAttempt(
    projectId: string,
    issueId: string,
    payload: CreateWorkflowAttemptPayload
  ): Promise<WorkflowAttemptResponse> {
    return mutate(
      await localFetch(
        `/projects/${projectId}/issues/${issueId}/workflow-attempts`,
        {
          method: 'POST',
          body: JSON.stringify(payload),
        }
      ),
      'Failed to create workflow attempt'
    );
  },

  async getAttempt(attemptId: string): Promise<WorkflowAttemptResponse> {
    return getJson(
      await localFetch(`/workflow-attempts/${attemptId}`),
      'Failed to get workflow attempt'
    );
  },

  async deleteAttempt(attemptId: string): Promise<void> {
    const response = await localFetch(`/workflow-attempts/${attemptId}`, {
      method: 'DELETE',
    });
    if (!response.ok) {
      throw await parseError(response, 'Failed to delete workflow attempt');
    }
    await response.json();
  },

  async getAttemptForWorkflow(
    workflowId: string
  ): Promise<WorkflowAttemptResponse | null> {
    return getJson(
      await localFetch(`/workflows/${workflowId}/attempt`),
      'Failed to get workflow attempt'
    );
  },

  async runAttempt(
    attemptId: string,
    payload: RunWorkflowAttemptPayload
  ): Promise<WorkflowRunResponse> {
    return mutate(
      await localFetch(`/workflow-attempts/${attemptId}/run`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to run workflow attempt'
    );
  },

  async getRun(runId: string): Promise<WorkflowRunResponse> {
    return getJson(
      await localFetch(`/workflow-runs/${runId}`),
      'Failed to get workflow run'
    );
  },

  async getRepositoryRunForWorkspace(
    workspaceId: string
  ): Promise<WorkflowRunResponse | null> {
    return getJson(
      await localFetch(`/workspaces/${workspaceId}/repository-workflow-run`),
      'Failed to get repository workflow run'
    );
  },

  async cancelRun(runId: string): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(`/workflow-runs/${runId}/cancel`, {
        method: 'POST',
      }),
      'Failed to cancel workflow run'
    );
  },

  async approve(
    runId: string,
    nodeId: string,
    payload: ApproveNodeRequest
  ): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(`/workflow-runs/${runId}/nodes/${nodeId}/approve`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to approve node'
    );
  },

  async reject(
    runId: string,
    nodeId: string,
    payload: RejectNodeRequest
  ): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(`/workflow-runs/${runId}/nodes/${nodeId}/reject`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to reject node'
    );
  },

  async retry(runId: string, nodeId: string): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(`/workflow-runs/${runId}/nodes/${nodeId}/retry`, {
        method: 'POST',
      }),
      'Failed to retry node'
    );
  },

  async selectArenaWinner(
    runId: string,
    nodeId: string,
    payload: SelectArenaWinnerRequest
  ): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(`/workflow-runs/${runId}/nodes/${nodeId}/arena-winner`, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
      'Failed to select arena winner'
    );
  },

  async selectConditionBranch(
    runId: string,
    nodeId: string,
    payload: SelectConditionBranchRequest
  ): Promise<WorkflowActionResponse> {
    return mutate(
      await localFetch(
        `/workflow-runs/${runId}/nodes/${nodeId}/condition-branch`,
        {
          method: 'POST',
          body: JSON.stringify(payload),
        }
      ),
      'Failed to select condition branch'
    );
  },
};

export const workflowPresetsApi = {
  async defaults(projectId: string): Promise<PresetDefaults[]> {
    return getJson(
      await localFetch(`/projects/${projectId}/workflow-preset-defaults`),
      'Failed to load preset defaults'
    );
  },
  async save(projectId: string, value: PresetDefaults): Promise<void> {
    await getJson(
      await localFetch(`/projects/${projectId}/workflow-preset-defaults`, {
        method: 'PUT',
        body: JSON.stringify(value),
      }),
      'Failed to save preset defaults'
    );
  },
  async start(value: StartPresetRequest): Promise<WorkflowRunResponse> {
    return getJson(
      await localFetch('/workflow-presets/start', {
        method: 'POST',
        body: JSON.stringify(value),
      }),
      'Failed to start Workflow'
    );
  },
};
