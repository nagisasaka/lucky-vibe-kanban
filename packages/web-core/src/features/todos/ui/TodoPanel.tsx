import { useId, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { DotsThreeIcon, ArrowSquareOutIcon } from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@vibe/ui/components/Dropdown';
import type { Todo, UpdateTodo } from 'shared/types';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { cn } from '@/shared/lib/utils';
import { TodoBody } from './TodoBody';
import { todoApi } from '../model/todoApi';
import { useTodoViewStore } from '../model/todoViewStore';

const field =
  'w-full min-w-0 rounded-sm border border-border bg-primary px-base py-half text-normal focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand';

export function TodoPanel({
  projectId,
  grid = false,
}: {
  projectId: string;
  grid?: boolean;
}) {
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const categoriesId = useId();
  const queryKey = ['project-todos', projectId];
  const query = useQuery({
    queryKey,
    queryFn: () => todoApi.list(projectId),
    refetchInterval: 5000,
  });
  const [showCompleted, setShowCompleted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const draft = useTodoViewStore((s) => s.drafts[projectId]);
  const setDraft = useTodoViewStore((s) => s.setDraft);
  const clearDraft = useTodoViewStore((s) => s.clearDraft);
  const changeDraft = (change: { body?: string; category?: string }) =>
    setDraft(projectId, {
      id: draft?.id ?? crypto.randomUUID(),
      body: draft?.body ?? '',
      category: draft?.category ?? '',
      ...change,
    });
  const submitting = useRef(false);
  const create = useMutation({
    mutationFn: () =>
      todoApi.create(projectId, {
        id: draft!.id,
        body: draft!.body,
        category: draft!.category,
        issue_id: null,
      }),
    onSuccess: () => {
      clearDraft(projectId);
      setError(null);
      void queryClient.invalidateQueries({ queryKey });
    },
    onError: (e: Error) => setError(e.message),
    onSettled: () => {
      submitting.current = false;
    },
  });
  const submit = () => {
    if (!draft?.body.trim() || submitting.current || create.isPending) return;
    if (!draft.category.trim()) {
      setError(t('todo.categoryRequired'));
      return;
    }
    setError(null);
    submitting.current = true;
    create.mutate();
  };
  const groups = useMemo(() => {
    const grouped = new Map<string, Todo[]>();
    for (const item of query.data ?? []) {
      if (item.completed && !showCompleted) continue;
      const entries = grouped.get(item.category) ?? [];
      entries.push(item);
      grouped.set(item.category, entries);
    }
    return [...grouped.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [query.data, showCompleted]);
  const categories = [
    ...new Set((query.data ?? []).map((item) => item.category).filter(Boolean)),
  ].sort();
  return (
    <section
      aria-label={t('todo.title')}
      className="flex h-full min-h-0 flex-col text-normal"
      data-testid={grid ? 'todo-grid' : 'todo-list'}
    >
      <div className="shrink-0 space-y-base border-b border-border p-double">
        <form
          className="space-y-base"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <textarea
            aria-label={t('todo.newBody')}
            placeholder={t('todo.newBody')}
            rows={2}
            maxLength={16000}
            className={cn(field, 'resize-y')}
            value={draft?.body ?? ''}
            disabled={create.isPending}
            onChange={(event) => changeDraft({ body: event.target.value })}
            onKeyDown={(event) => {
              if (
                event.key !== 'Enter' ||
                event.nativeEvent.isComposing ||
                event.nativeEvent.keyCode === 229
              )
                return;
              if (event.ctrlKey) {
                event.preventDefault();
                const input = event.currentTarget;
                const start = input.selectionStart;
                const end = input.selectionEnd;
                if (input.value.length - (end - start) >= 16000) return;
                changeDraft({
                  body:
                    input.value.slice(0, start) + '\n' + input.value.slice(end),
                });
                requestAnimationFrame(() =>
                  input.setSelectionRange(start + 1, start + 1)
                );
                return;
              }
              if (event.shiftKey || event.metaKey || event.altKey) return;
              event.preventDefault();
              if (!event.repeat) submit();
            }}
          />
          <p className="text-xs text-low">{t('todo.inputHint')}</p>
          <div className="flex flex-wrap items-end gap-base">
            <label className="min-w-0 flex-1 text-sm text-low">
              {t('todo.category')}
              <input
                list={categoriesId}
                placeholder={t('todo.uncategorized')}
                maxLength={100}
                className={field}
                value={draft?.category ?? ''}
                disabled={create.isPending}
                onChange={(event) =>
                  changeDraft({ category: event.target.value })
                }
              />
            </label>
            <datalist id={categoriesId}>
              {categories.map((category) => (
                <option key={category} value={category} />
              ))}
            </datalist>
            <Button
              type="submit"
              size="sm"
              disabled={!draft?.body.trim() || create.isPending}
            >
              {t('todo.add')}
            </Button>
          </div>
        </form>
        <label className="flex items-center gap-base text-sm">
          <input
            type="checkbox"
            checked={showCompleted}
            onChange={(event) => setShowCompleted(event.target.checked)}
          />
          {t('todo.showCompleted')}
        </label>
        {(error || query.error) && (
          <p role="alert" className="text-error text-sm break-words">
            {error || query.error?.message}
          </p>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-double">
        {query.isLoading ? (
          <p className="text-low">{t('states.loading')}</p>
        ) : !query.isError && groups.length === 0 ? (
          <p className="py-double text-low">{t('todo.empty')}</p>
        ) : null}
        <TodoCategoryLayout grid={grid}>
          {groups.map(([category, items]) => (
            <div key={category} className="min-w-0">
              <div className="pb-double">
                <section className="min-w-0 overflow-hidden rounded-sm border border-border bg-secondary">
                  <h3 className="border-b border-border px-base py-base font-medium text-high break-words">
                    {category || t('todo.uncategorized')}{' '}
                    <span className="ml-half text-xs font-normal text-low">
                      {items.length}
                    </span>
                  </h3>
                  <ul className="divide-y divide-border">
                    {items.map((item) => (
                      <TodoRow
                        key={item.id}
                        todo={item}
                        categories={categories}
                      />
                    ))}
                  </ul>
                </section>
              </div>
            </div>
          ))}
        </TodoCategoryLayout>
      </div>
    </section>
  );
}

// Sparse row placement keeps category DOM/keyboard order and top-to-bottom
// visual order. Do not use dense packing, which can move later categories ahead.
function TodoCategoryLayout({
  grid,
  children,
}: {
  grid: boolean;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const container = ref.current;
    if (!container || !grid) return;
    const measure = (content: Element) => {
      const cell = content.parentElement!;
      cell.style.gridRowEnd = `span ${Math.max(
        1,
        Math.ceil(content.getBoundingClientRect().height)
      )}`;
    };
    // Observe natural content height, not the grid cell's allocated row height.
    // This also handles text wrapping, editing, completion and font-size changes.
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) measure(entry.target);
    });
    for (const cell of container.children) {
      const content = cell.firstElementChild!;
      measure(content);
      observer.observe(content);
    }
    return () => {
      observer.disconnect();
      for (const cell of container.children) {
        (cell as HTMLElement).style.removeProperty('grid-row-end');
      }
    };
  }, [grid, children]);

  return (
    <div
      ref={ref}
      data-testid="todo-categories"
      className={cn(
        'grid items-start gap-x-double',
        grid
          ? 'auto-rows-[1px] grid-cols-[repeat(auto-fit,minmax(min(100%,320px),1fr))]'
          : 'grid-cols-1'
      )}
    >
      {children}
    </div>
  );
}

function TodoRow({ todo, categories }: { todo: Todo; categories: string[] }) {
  const { t } = useTranslation('common');
  const navigation = useAppNavigation();
  const setProjectView = useTodoViewStore((s) => s.setProjectView);
  const queryClient = useQueryClient();
  const categoryId = useId();
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [body, setBody] = useState(todo.body);
  const [category, setCategory] = useState(todo.category);
  const [editRevision, setEditRevision] = useState(todo.revision);
  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: async (
      changes: (Partial<UpdateTodo> & { expected_revision: number }) | 'delete'
    ) => {
      if (changes === 'delete') await todoApi.remove(todo.project_id, todo);
      else await todoApi.update(todo.project_id, todo.id, changes);
    },
    onSuccess: () => {
      setEditing(false);
      setConfirmDelete(false);
      setError(null);
      void queryClient.invalidateQueries({
        queryKey: ['project-todos', todo.project_id],
      });
    },
    onError: (e: Error) => {
      setError(e.message);
      void queryClient.invalidateQueries({
        queryKey: ['project-todos', todo.project_id],
      });
    },
  });
  const edit = () => {
    setBody(todo.body);
    setCategory(todo.category);
    setEditRevision(todo.revision);
    setError(null);
    setEditing(true);
  };
  return (
    <li className="min-w-0 p-base" data-todo-id={todo.id}>
      {editing ? (
        <form
          className="space-y-base"
          onSubmit={(event) => {
            event.preventDefault();
            if (body.trim() && !mutation.isPending)
              mutation.mutate({
                expected_revision: editRevision,
                body,
                category,
              });
          }}
        >
          <textarea
            aria-label={t('todo.editBody')}
            rows={4}
            maxLength={16000}
            className={field}
            value={body}
            onChange={(event) => setBody(event.target.value)}
            disabled={mutation.isPending}
          />
          <input
            aria-label={t('todo.category')}
            list={categoryId}
            maxLength={100}
            className={field}
            value={category}
            onChange={(event) => setCategory(event.target.value)}
            disabled={mutation.isPending}
          />
          <datalist id={categoryId}>
            {categories.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
          <div className="flex gap-base">
            <Button
              size="sm"
              type="submit"
              disabled={!body.trim() || mutation.isPending}
            >
              {t('todo.save')}
            </Button>
            <Button
              size="sm"
              type="button"
              disabled={mutation.isPending}
              onClick={() => {
                setEditing(false);
                setError(null);
              }}
            >
              {t('todo.cancel')}
            </Button>
          </div>
        </form>
      ) : (
        <div className="flex items-start gap-base">
          <input
            type="checkbox"
            className="mt-half shrink-0"
            aria-label={t(
              todo.completed ? 'todo.markIncomplete' : 'todo.markComplete',
              { body: todo.body }
            )}
            checked={todo.completed}
            disabled={mutation.isPending}
            onChange={(event) =>
              mutation.mutate({
                expected_revision: todo.revision,
                completed: event.target.checked,
              })
            }
          />
          <div className="min-w-0 flex-1">
            <TodoBody
              body={todo.body}
              completed={todo.completed}
              disabled={mutation.isPending}
              onEdit={edit}
            />
            <span
              className="mt-half block text-xs text-low"
              title={t('todo.creationSourceHint')}
              data-creation-source={todo.creation_source}
            >
              {t('todo.creationSource', {
                source:
                  todo.creation_source === 'ui'
                    ? 'UI'
                    : todo.creation_source === 'mcp'
                      ? 'MCP'
                      : t('todo.creationSourceUnknown'),
              })}
            </span>
            {todo.issue_id && (
              <button
                type="button"
                className="mt-half flex items-center gap-half text-xs text-brand hover:underline"
                onClick={() => {
                  setProjectView(todo.project_id, 'kanban');
                  navigation.goToProjectIssue(todo.project_id, todo.issue_id!);
                }}
              >
                <ArrowSquareOutIcon className="size-icon-xs" />
                {t('todo.relatedCard')}
              </button>
            )}
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                aria-label={t('todo.itemActions')}
                disabled={mutation.isPending}
                className="shrink-0 rounded-sm p-half text-low hover:bg-panel"
              >
                <DotsThreeIcon className="size-icon-sm" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={edit}>
                {t('todo.edit')}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setConfirmDelete(true)}>
                {t('todo.delete')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}
      {confirmDelete && (
        <div className="mt-base space-y-base text-sm">
          <p>{t('todo.confirmDelete')}</p>
          <div className="flex gap-base">
            <Button
              size="sm"
              variant="destructive"
              disabled={mutation.isPending}
              onClick={() => mutation.mutate('delete')}
            >
              {t('todo.delete')}
            </Button>
            <Button
              size="sm"
              disabled={mutation.isPending}
              onClick={() => setConfirmDelete(false)}
            >
              {t('todo.cancel')}
            </Button>
          </div>
        </div>
      )}
      {error && (
        <p role="alert" className="mt-base text-error text-sm break-words">
          {error}
        </p>
      )}
    </li>
  );
}
