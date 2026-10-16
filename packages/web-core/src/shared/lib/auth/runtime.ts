import { LOCAL_USER_ID } from '@/shared/lib/localIdentity';
export function getAuthRuntime() {
  return { getCurrentUser: async () => ({ user_id: LOCAL_USER_ID }) };
}
