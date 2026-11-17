import { expect, it, vi } from 'vitest';
import { mustKeepProjectTextInert } from './useImportedProject';
import type { TransferReport } from '@/shared/lib/projectTransferApi';
vi.mock('@/shared/lib/projectTransferApi', () => ({ projectTransferApi: {} }));
it('fails closed before lookup, after errors and for imported records; only confirmed ordinary Projects enable rich links', () => {
  expect(mustKeepProjectTextInert('pending', undefined)).toBe(true);
  expect(mustKeepProjectTextInert('error', undefined)).toBe(true);
  expect(mustKeepProjectTextInert('error', null)).toBe(true);
  expect(mustKeepProjectTextInert('success', {} as TransferReport)).toBe(true);
  expect(mustKeepProjectTextInert('success', undefined)).toBe(true);
  expect(mustKeepProjectTextInert('success', null)).toBe(false);
});
