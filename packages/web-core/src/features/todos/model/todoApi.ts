import type { Todo, TodoPage, CreateTodo, UpdateTodo } from 'shared/types';
import { handleApiResponse } from '@/shared/lib/api';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  return handleApiResponse<T>(
    await makeLocalApiRequest(path, {
      ...init,
      headers: { 'Content-Type': 'application/json', ...init?.headers },
    })
  );
}
const path = (project: string) => `/api/projects/${project}/todos`;
export const todoApi = {
  async list(project: string): Promise<Todo[]> {
    const items: Todo[] = [];
    let offset: number | null = 0;
    do {
      const page: TodoPage = await request(
        `${path(project)}?include_completed=true&limit=200&offset=${offset}`
      );
      items.push(...page.items);
      offset = page.next_offset;
    } while (offset !== null);
    return items;
  },
  create: (project: string, input: CreateTodo) =>
    request<Todo>(path(project), {
      method: 'POST',
      body: JSON.stringify(input),
    }),
  update: (
    project: string,
    id: string,
    input: Partial<UpdateTodo> & { expected_revision: number }
  ) =>
    request<Todo>(`${path(project)}/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(input),
    }),
  remove: (project: string, todo: Todo) =>
    request<void>(
      `${path(project)}/${todo.id}?expected_revision=${todo.revision}`,
      { method: 'DELETE' }
    ),
  workspaceProject: (workspace: string) =>
    request<string | null>(`/api/workspaces/${workspace}/todo-project`),
};
