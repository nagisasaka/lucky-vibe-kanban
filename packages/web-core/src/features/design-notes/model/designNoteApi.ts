import type {
  DesignNote,
  DesignNoteAddition,
  DesignNoteDetail,
  DesignNoteWrite,
  DesignNoteResult,
} from 'shared/types';
import { handleApiResponse } from '@/shared/lib/api';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
const path = (project: string) => `/api/projects/${project}/design-notes`;
async function request<T>(url: string, init?: RequestInit): Promise<T> {
  return handleApiResponse<T>(
    await makeLocalApiRequest(url, {
      ...init,
      headers: { 'Content-Type': 'application/json' },
    })
  );
}
async function pages<T>(url: string): Promise<T[]> {
  const result: T[] = [];
  for (let offset = 0; ; offset += 100) {
    const page = await request<T[]>(`${url}?offset=${offset}`);
    result.push(...page);
    if (page.length < 100) return result;
  }
}
export const designNoteApi = {
  list: (p: string) => pages<DesignNote>(path(p)),
  get: (p: string, id: string) => request<DesignNoteDetail>(`${path(p)}/${id}`),
  history: (p: string, id: string) =>
    pages<DesignNote>(`${path(p)}/${id}/history`),
  additionHistory: (p: string, id: string, a: string) =>
    pages<DesignNoteAddition>(`${path(p)}/${id}/additions/${a}/history`),
  write: (p: string, input: DesignNoteWrite) =>
    request<DesignNoteResult>(path(p), {
      method: 'POST',
      body: JSON.stringify(input),
    }),
};
