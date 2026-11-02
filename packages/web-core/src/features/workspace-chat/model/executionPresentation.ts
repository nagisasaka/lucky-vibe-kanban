import type { TFunction } from 'i18next';

export function executionStatusLabel(t: TFunction<'tasks'>, status: string) {
  switch (status) {
    case 'unknown':
      return t('tasks:executionInspection.status.unknown');
    case 'pending':
      return t('tasks:executionInspection.status.pending');
    case 'queued':
      return t('tasks:executionInspection.status.queued');
    case 'starting':
      return t('tasks:executionInspection.status.starting');
    case 'running':
      return t('tasks:executionInspection.status.running');
    case 'awaiting_input':
      return t('tasks:executionInspection.status.awaiting_input');
    case 'awaiting_approval':
      return t('tasks:executionInspection.status.awaiting_approval');
    case 'cancelling':
      return t('tasks:executionInspection.status.cancelling');
    case 'succeeded':
      return t('tasks:executionInspection.status.succeeded');
    case 'failed':
      return t('tasks:executionInspection.status.failed');
    case 'cancelled':
      return t('tasks:executionInspection.status.cancelled');
    case 'canceled':
      return t('tasks:executionInspection.status.canceled');
    case 'crashed':
      return t('tasks:executionInspection.status.crashed');
    case 'audit_failed':
      return t('tasks:executionInspection.status.audit_failed');
    case 'preparing':
      return t('tasks:executionInspection.status.preparing');
    case 'integrating':
      return t('tasks:executionInspection.status.integrating');
    case 'validating':
      return t('tasks:executionInspection.status.validating');
    case 'publishing':
      return t('tasks:executionInspection.status.publishing');
    case 'finalizing':
      return t('tasks:executionInspection.status.finalizing');
    case 'recovery_required':
      return t('tasks:executionInspection.status.recovery_required');
    case 'blocked':
      return t('tasks:executionInspection.status.blocked');
    case 'applied':
      return t('tasks:executionInspection.status.applied');
    case 'stale':
      return t('tasks:executionInspection.status.stale');
    case 'admitted':
      return t('tasks:executionInspection.status.admitted');
    case 'cleaningup':
      return t('tasks:executionInspection.status.cleaningup');
    case 'sourcing':
      return t('tasks:executionInspection.status.sourcing');
    case 'analyzing':
      return t('tasks:executionInspection.status.analyzing');
    case 'generating':
      return t('tasks:executionInspection.status.generating');
    case 'verifying':
      return t('tasks:executionInspection.status.verifying');
    default:
      return t('tasks:executionInspection.statusOriginal', { status });
  }
}

export function executionOwnerLabel(
  t: TFunction<'tasks'>,
  owner: string | undefined
) {
  switch (owner) {
    case 'integration':
      return t('tasks:executionInspection.owner.integration');
    case 'resource_mediation':
      return t('tasks:executionInspection.owner.resource_mediation');
    case 'openwiki_sync':
      return t('tasks:executionInspection.owner.openwiki_sync');
    case 'openwiki_bootstrap':
      return t('tasks:executionInspection.owner.openwiki_bootstrap');
    case undefined:
      return t('tasks:executionInspection.unknownOwner');
    default:
      return t('tasks:executionInspection.ownerOriginal', { owner });
  }
}
