import type { TFunction } from 'i18next';
import type {
  RegisterResource,
  ResourceEvent,
  ResourceMediation,
  ResourceOperation,
  ResourceRecovery,
  ResourceSnapshot,
  SharedResource,
} from 'shared/types';
import { handleApiResponse } from './api';

export function createResourceCoordinationClient(
  send: (path: string, init?: RequestInit) => Promise<Response>
) {
  async function request<T>(path: string, body?: unknown): Promise<T> {
    return handleApiResponse<T>(
      await send(
        `/api/resource-coordination${path}`,
        body === undefined
          ? { cache: 'no-store' }
          : {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(body),
            }
      )
    );
  }
  return {
    snapshot: () => request<ResourceSnapshot>('/snapshot'),
    operations: () => request<ResourceOperation[]>('/operations'),
    events: (after = 0) => request<ResourceEvent[]>(`/events?after=${after}`),
    mediations: () => request<ResourceMediation[]>('/mediations'),
    stopMediation: (id: string) =>
      request<void>(`/mediations/${id}/cancel`, {}),
    register: (body: RegisterResource) =>
      request<SharedResource>('/resources', body),
    cancel: (id: string) =>
      request<ResourceOperation>(`/operations/${id}/cancel`, {}),
    mediate: (id: string) =>
      request<ResourceMediation | null>(`/operations/${id}/mediate`, {}),
    recover: (id: string, body: ResourceRecovery) =>
      request<ResourceOperation>(`/operations/${id}/recover`, body),
  };
}

export type ResourceCoordinationClient = ReturnType<
  typeof createResourceCoordinationClient
>;

export function resourceOperationCanCancel(status: string) {
  return [
    'queued',
    'blocked',
    'launching',
    'running',
    'recovery_required',
  ].includes(status);
}

/** API status strings stay unchanged; only known presentation labels are translated. */
export function resourceStatusLabel(
  kind: 'health' | 'operation' | 'mediation',
  value: string,
  t: TFunction<'settings'>
): string {
  if (kind === 'health') {
    switch (value) {
      case 'ready':
        return t('settings:resources.health.ready');
      case 'recovery_required':
        return t('settings:resources.health.recovery_required');
    }
  }
  if (kind === 'operation') {
    switch (value) {
      case 'queued':
        return t('settings:resources.status.queued');
      case 'blocked':
        return t('settings:resources.status.blocked');
      case 'launching':
        return t('settings:resources.status.launching');
      case 'running':
        return t('settings:resources.status.running');
      case 'succeeded':
        return t('settings:resources.status.succeeded');
      case 'cancelled':
        return t('settings:resources.status.cancelled');
      case 'recovery_required':
        return t('settings:resources.status.recovery_required');
      case 'recovered':
        return t('settings:resources.status.recovered');
    }
  }
  if (kind === 'mediation') {
    switch (value) {
      case 'pending':
        return t('settings:resources.mediationStatus.pending');
      case 'preparing':
        return t('settings:resources.mediationStatus.preparing');
      case 'running':
        return t('settings:resources.mediationStatus.running');
      case 'applied':
        return t('settings:resources.mediationStatus.applied');
      case 'stale':
        return t('settings:resources.mediationStatus.stale');
      case 'failed':
        return t('settings:resources.mediationStatus.failed');
    }
  }
  return t('settings:resources.unknownState');
}
