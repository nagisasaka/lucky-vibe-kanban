import type { RuntimeAccessView, SaveRuntimeAccess } from 'shared/types';
import { makeLocalApiRequest } from './localApiTransport';

export async function runtimeAccessSettings(
  input?: SaveRuntimeAccess
): Promise<RuntimeAccessView> {
  const response = await makeLocalApiRequest('/api/runtime-access-settings', {
    method: input ? 'PUT' : 'GET',
    headers: { 'Content-Type': 'application/json' },
    ...(input ? { body: JSON.stringify(input) } : {}),
  });
  if (!response.ok) throw new Error(await response.text());
  return response.json();
}
