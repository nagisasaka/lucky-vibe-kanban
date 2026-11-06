import type { Notification } from 'shared/remote-types';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import type { GroupedNotification } from '@/shared/lib/notifications';
import type { MutationResult, SyncError } from '@/shared/lib/electric/types';

const EMPTY_NOTIFICATIONS: Notification[] = [];
const EMPTY_GROUPS: GroupedNotification[] = [];
const retry = () => {};
const updateMany: (
  updates: Array<{ id: string; changes: { seen?: boolean } }>
) => MutationResult = () => ({ persisted: Promise.resolve() });

// The local notification inbox is empty. Run approvals/questions and Web Push
// use separate APIs; keep the inbox page and bell's existing empty-state contract.
export function useNotifications() {
  const { isSignedIn, userId } = useAuth();
  return {
    data: EMPTY_NOTIFICATIONS,
    enabled: isSignedIn && !!userId,
    groupedNotifications: EMPTY_GROUPS,
    unseenCount: 0,
    isLoading: false,
    error: null as SyncError | null,
    retry,
    updateMany,
  };
}
