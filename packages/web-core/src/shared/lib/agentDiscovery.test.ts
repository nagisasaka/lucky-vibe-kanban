import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import { agentsApi } from './api';

describe('selected profile discovery requests', () => {
  it('keeps Claude subscription selection and Workspace scope on the stream', () => {
    const url = new URL(
      agentsApi.getDiscoveredOptionsStreamUrl(BaseCodingAgent.CLAUDE_CODE, {
        variant: 'SUBSCRIPTION',
        workspaceId: 'workspace',
        sessionId: 'session',
      }),
      'http://localhost'
    );
    expect(Object.fromEntries(url.searchParams)).toEqual({
      executor: 'CLAUDE_CODE',
      variant: 'SUBSCRIPTION',
      workspace_id: 'workspace',
      session_id: 'session',
    });
  });

  it('keeps Workspace cwd for Claude discovery before a Session exists', () => {
    const url = new URL(
      agentsApi.getDiscoveredOptionsStreamUrl(BaseCodingAgent.CLAUDE_CODE, {
        variant: 'SUBSCRIPTION',
        workspaceId: 'workspace',
      }),
      'http://localhost'
    );
    expect(url.searchParams.get('workspace_id')).toBe('workspace');
    expect(url.searchParams.has('session_id')).toBe(false);
  });

  it('preserves the existing default Codex request', () => {
    expect(agentsApi.getDiscoveredOptionsStreamUrl(BaseCodingAgent.CODEX)).toBe(
      '/api/agents/discovered-options/ws?executor=CODEX'
    );
  });
});
