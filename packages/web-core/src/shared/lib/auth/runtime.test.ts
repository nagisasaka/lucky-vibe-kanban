import { afterEach, expect, it } from 'vitest';
import {
  LOCAL_USER_ID,
  setLocalServerUserId,
} from '@/shared/lib/localIdentity';
import { getAuthRuntime } from './runtime';

afterEach(() => setLocalServerUserId(LOCAL_USER_ID));

it('uses the authenticated server identity after the access gate loads', async () => {
  const runtime = getAuthRuntime();
  expect(await runtime.getCurrentUser()).toEqual({ user_id: LOCAL_USER_ID });

  const userId = '00000000-0000-0000-0000-000000000123';
  setLocalServerUserId(userId);
  expect(await runtime.getCurrentUser()).toEqual({ user_id: userId });
});
