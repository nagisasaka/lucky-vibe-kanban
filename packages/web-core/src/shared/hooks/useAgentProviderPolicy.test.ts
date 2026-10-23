import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AgentProviderReadiness,
  BaseCodingAgent,
  type AgentProviderPolicy,
} from 'shared/types';
import { useAgentProviderPolicy } from './useAgentProviderPolicy';

const fixture = vi.hoisted(() => ({
  policy: undefined as AgentProviderPolicy | undefined,
  error: false,
  queries: [] as Array<{
    queryKey: unknown[];
    enabled: boolean;
    queryFn: () => unknown;
  }>,
  getProfilePolicy: vi.fn(),
}));

vi.mock('@/shared/hooks/useUserSystem', () => ({ useUserSystem: vi.fn() }));
vi.mock('@/shared/lib/api', () => ({
  agentsApi: { getGarage: vi.fn(), getProfilePolicy: fixture.getProfilePolicy },
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: (query: (typeof fixture.queries)[number]) => {
    fixture.queries.push(query);
    return query.queryKey[1] === 'profile'
      ? { data: fixture.policy, isError: fixture.error }
      : { data: [{ executor: 'CODEX', policy: { readiness: 'READY' } }] };
  },
}));

function readPolicy(executor: BaseCodingAgent, variant: string) {
  let result: AgentProviderPolicy | null = null;
  function Probe() {
    result = useAgentProviderPolicy(executor, variant);
    return null;
  }
  renderToStaticMarkup(createElement(Probe));
  return result as AgentProviderPolicy | null;
}

describe('selected profile readiness', () => {
  beforeEach(() => {
    fixture.policy = undefined;
    fixture.error = false;
    fixture.queries = [];
    fixture.getProfilePolicy.mockReset();
  });

  it('uses the selected Claude profile and preserves its auth diagnostic', () => {
    fixture.policy = {
      executor: BaseCodingAgent.CLAUDE_CODE,
      readiness: AgentProviderReadiness.AUTH_REQUIRED,
      capabilities: [],
      legacy: false,
      disabled: false,
      diagnostics: [],
    };
    expect(readPolicy(BaseCodingAgent.CLAUDE_CODE, 'SUBSCRIPTION')).toBe(
      fixture.policy
    );
    const query = fixture.queries.find((q) => q.queryKey[1] === 'profile')!;
    expect(query.enabled).toBe(true);
    expect(query.queryKey).toContain('SUBSCRIPTION');
    query.queryFn();
    expect(fixture.getProfilePolicy).toHaveBeenCalledWith({
      executor: 'CLAUDE_CODE',
      variant: 'SUBSCRIPTION',
      workspace_id: undefined,
      session_id: undefined,
    });
  });

  it('never substitutes a provider-wide ready result when the profile check fails', () => {
    fixture.error = true;
    expect(
      readPolicy(BaseCodingAgent.CLAUDE_CODE, 'SUBSCRIPTION')?.readiness
    ).toBe(AgentProviderReadiness.UNKNOWN);
  });

  it('keeps Codex on the existing garage path without a Claude probe', () => {
    expect(readPolicy(BaseCodingAgent.CODEX, 'DEFAULT')?.readiness).toBe(
      AgentProviderReadiness.READY
    );
    expect(
      fixture.queries.find((q) => q.queryKey[1] === 'profile')?.enabled
    ).toBe(false);
  });
});
