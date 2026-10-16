import { currentLocalUserId } from '@/shared/lib/localIdentity';
export function getAuthRuntime() {
  return { getCurrentUser: async () => ({ user_id: currentLocalUserId() }) };
}
