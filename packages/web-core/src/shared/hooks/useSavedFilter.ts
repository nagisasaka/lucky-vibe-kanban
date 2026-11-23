import { useEffect, useRef, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import type { SavedFilterEvaluation } from 'shared/remote-types';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import {
  savedFiltersApi,
  savedFilterKeys,
  SavedFilterRequestError,
} from '@/shared/lib/savedFiltersApi';
import { hasEvaluationResult } from '@/shared/lib/savedFilters';

export function useSavedFilter() {
  const project = useProjectContext();
  const search = useRouterState({
    select: (state) => state.location.search as { savedFilter?: string },
  });
  const id = search.savedFilter;
  const navigate = useNavigate();
  const selected = id !== undefined;
  const list = useQuery({
    queryKey: savedFilterKeys.list(project.projectId),
    queryFn: ({ signal }) => savedFiltersApi.list(project.projectId, signal),
    refetchInterval: 15000,
  });
  const definition = useQuery({
    queryKey: savedFilterKeys.definition(project.projectId, id ?? ''),
    queryFn: ({ signal }) => {
      if (
        !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          id ?? ''
        )
      ) {
        throw new SavedFilterRequestError('Invalid saved filter ID', 400);
      }
      return savedFiltersApi.get(project.projectId, id!, signal);
    },
    enabled: selected,
    retry: false,
    refetchInterval: selected ? 15000 : false,
  });
  // Shapes cover all project issues, including archived issues and noncandidates.
  // Serialize the evidence so equal periodic snapshots do not trigger extra calls.
  const evidence = JSON.stringify([
    project.issues,
    project.issueTags,
    project.issueAssignees,
    project.issueRelationships,
    project.tags,
    project.statuses,
    definition.data?.revision,
  ]);
  const evaluatedEvidence = useRef<string | null>(null);
  const retryFailed = useRef(false);
  const evaluation = useQuery({
    queryKey: savedFilterKeys.evaluation(project.projectId, id ?? ''),
    queryFn: async ({ signal }) => {
      const retry = retryFailed.current;
      retryFailed.current = false;
      const result = await savedFiltersApi.evaluate(
        project.projectId,
        id!,
        signal,
        retry
      );
      if (!signal.aborted) evaluatedEvidence.current = evidence;
      return result;
    },
    enabled: selected && !!definition.data && !definition.isError,
    retry: false,
    refetchOnMount: 'always',
    refetchInterval: (query) =>
      selected && !definition.isError
        ? query.state.data?.state === 'evaluating'
          ? 2000
          : 15000
        : false,
  });
  const refetch = evaluation.refetch;
  useEffect(() => {
    if (!selected || !definition.data || definition.isError) return;
    const timer = setTimeout(() => void refetch(), 300);
    return () => clearTimeout(timer);
  }, [evidence, selected, definition.data, definition.isError, refetch]);
  const previous = useRef<{
    key: string;
    result: SavedFilterEvaluation;
  } | null>(null);
  const key = `${project.projectId}/${id}`;
  const result = evaluation.data;
  useEffect(() => {
    if (hasEvaluationResult(result))
      previous.current = { key, result: result! };
  }, [key, result]);
  const display =
    !definition.isError && selected
      ? hasEvaluationResult(result)
        ? result
        : previous.current?.key === key
          ? previous.current.result
          : undefined
      : undefined;
  const issueIds = useMemo(
    () => (display ? new Set(display.issue_ids) : null),
    [display]
  );
  const missingIssues =
    !!issueIds &&
    [...issueIds].some((issueId) => !project.issuesById.has(issueId));
  const stale =
    !!display &&
    (evaluatedEvidence.current !== evidence ||
      display !== result ||
      evaluation.isFetching ||
      evaluation.isError ||
      result?.stale ||
      result?.definition_revision !== definition.data?.revision);
  return {
    id,
    selected,
    list,
    definition,
    evaluation,
    issueIds,
    missingIssues,
    stale,
    ready: !!display && !missingIssues,
    state: evaluation.isError
      ? ('failed' as const)
      : (result?.state ??
        (evaluation.isFetching
          ? ('evaluating' as const)
          : ('unevaluated' as const))),
    select: (next: string | undefined) =>
      void navigate({
        to: '.',
        search: (prev: Record<string, unknown>) => ({
          ...prev,
          savedFilter: next,
        }),
      }),
    refresh: async () => {
      const current = await definition.refetch();
      if (current.data && !current.error) {
        retryFailed.current = true;
        void refetch();
      }
    },
  };
}
export type SavedFilterView = ReturnType<typeof useSavedFilter>;
