import { createCollection } from '@tanstack/react-db';

import { makeRequest } from '@/shared/lib/remoteApi';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import type { CollectionConfig, SyncError } from '@/shared/lib/electric/types';

type ElectricRow = Record<string, unknown> & { [key: string]: unknown };

type SourceRuntime = {
  refreshers: Set<() => Promise<void>>;
};

type MutationFnParams = {
  transaction: {
    mutations: Array<{
      modified?: unknown;
      original?: unknown;
      key?: string;
      changes?: unknown;
    }>;
  };
};

type SyncParams = {
  collection: {
    isReady: () => boolean;
    onFirstReady: (callback: () => void) => void;
  };
  begin: () => void;
  write: (message: {
    type: 'insert' | 'update' | 'delete';
    value: ElectricRow;
    metadata?: Record<string, unknown>;
  }) => void;
  commit: () => void;
  markReady: () => void;
  truncate: () => void;
};

type LoadSubsetFn = (options: unknown) => true | Promise<void>;
type UnloadSubsetFn = (options: unknown) => void;

type SyncResult =
  | void
  | (() => void)
  | {
      cleanup?: () => void;
      loadSubset?: LoadSubsetFn;
      unloadSubset?: UnloadSubsetFn;
    };

const DEFAULT_GC_TIME_MS = 5 * 60 * 1000;
const FALLBACK_REFRESH_INTERVAL_MS = 30 * 1000;

const collectionCache = new Map<string, ReturnType<typeof createCollection>>();
const sourceRuntimes = new Map<string, SourceRuntime>();
const fallbackSnapshotCache = new Map<string, ElectricRow[]>();

class ErrorHandler {
  private lastErrorTime = 0;
  private lastErrorMessage = '';
  private consecutiveErrors = 0;
  private readonly baseDebounceMs = 1000;
  private readonly maxDebounceMs = 30000;

  shouldReport(message: string): boolean {
    const now = Date.now();
    const debounceMs = Math.min(
      this.baseDebounceMs * Math.pow(2, this.consecutiveErrors),
      this.maxDebounceMs
    );

    if (
      message === this.lastErrorMessage &&
      now - this.lastErrorTime < debounceMs
    ) {
      return false;
    }

    this.lastErrorTime = now;
    if (message === this.lastErrorMessage) {
      this.consecutiveErrors += 1;
    } else {
      this.consecutiveErrors = 0;
      this.lastErrorMessage = message;
    }

    return true;
  }
}

function buildUrl(template: string, params: Record<string, string>): string {
  return template.replace(/\{([a-zA-Z_][a-zA-Z0-9_]*)\}/g, (_, key: string) => {
    if (!(key in params)) throw new Error(`Missing API parameter: ${key}`);
    return encodeURIComponent(params[key]);
  });
}

function isPageVisible(): boolean {
  return (
    typeof document === 'undefined' || document.visibilityState !== 'hidden'
  );
}

function buildFallbackRequestPath(
  fallbackUrl: string,
  params: Record<string, string>
): string {
  const path = buildUrl(fallbackUrl, params);
  const query = new URLSearchParams();

  for (const [key, value] of Object.entries(params)) {
    if (!value) continue;
    query.set(key, value);
  }

  const queryString = query.toString();
  return queryString ? `${path}?${queryString}` : path;
}

function buildCollectionId(
  table: string,
  params: Record<string, string>,
  hasMutations: boolean
): string {
  const sortedParams = Object.keys(params)
    .sort()
    .map((key) => params[key])
    .join('-');

  const base = sortedParams ? `${table}-${sortedParams}` : table;
  return hasMutations ? `${base}-mut` : base;
}

function buildSourceKey(table: string, params: Record<string, string>): string {
  const sortedEntries = Object.entries(params).sort(([a], [b]) =>
    a.localeCompare(b)
  );
  if (sortedEntries.length === 0) {
    return table;
  }

  const values = sortedEntries
    .map(([key, value]) => `${key}=${value}`)
    .join('&');
  return `${table}?${values}`;
}

function getRowKey(item: Record<string, unknown>): string {
  if ('id' in item && item.id) {
    return String(item.id);
  }

  return Object.entries(item)
    .filter(([key]) => key.endsWith('_id'))
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([, value]) => String(value))
    .join('-');
}

function getOrCreateSourceRuntime(sourceKey: string): SourceRuntime {
  const existing = sourceRuntimes.get(sourceKey);
  if (existing) {
    return existing;
  }

  const created: SourceRuntime = {
    refreshers: new Set(),
  };
  sourceRuntimes.set(sourceKey, created);
  return created;
}

function registerFallbackRefresher(
  sourceKey: string,
  refresher: () => Promise<void>
): () => void {
  const runtime = getOrCreateSourceRuntime(sourceKey);
  runtime.refreshers.add(refresher);
  return () => {
    runtime.refreshers.delete(refresher);
  };
}

function invalidateFallbackCache(sourceKey: string): void {
  fallbackSnapshotCache.delete(sourceKey);
}

async function refreshFallbackSource(sourceKey: string): Promise<void> {
  const runtime = getOrCreateSourceRuntime(sourceKey);
  await Promise.all([...runtime.refreshers].map((refresher) => refresher()));
}

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}

function createErrorReporter(
  config?: CollectionConfig
): (error: SyncError) => void {
  const handler = new ErrorHandler();

  return (error: SyncError) => {
    if (!handler.shouldReport(error.message)) return;

    if (isPageVisible()) {
      console.error('Shape sync error:', error);
    }
    config?.onError?.(error);
  };
}

function applySnapshot(syncParams: SyncParams, rows: ElectricRow[]): void {
  syncParams.begin();
  syncParams.truncate();

  for (const row of rows) {
    syncParams.write({
      type: 'insert',
      value: row,
      metadata: {},
    });
  }

  syncParams.commit();
  syncParams.markReady();
}

function extractFallbackRows(
  payload: unknown,
  table: string
): Array<ElectricRow> {
  if (!payload || typeof payload !== 'object') {
    throw new Error(`Fallback response for "${table}" is not an object`);
  }

  const rows = (payload as Record<string, unknown>)[table];
  if (!Array.isArray(rows)) {
    throw new Error(`Fallback response missing "${table}" array`);
  }

  return rows as Array<ElectricRow>;
}

async function parseResponseError(
  response: Response,
  fallbackMessage: string
): Promise<string> {
  const withStatus = (message: string) =>
    `${message} (${response.status} ${response.statusText})`;
  const textResponse = response.clone();

  try {
    const body = (await response.json()) as {
      message?: string;
      error?: string;
    };
    return withStatus(body.message || body.error || fallbackMessage);
  } catch {
    try {
      const bodyText = (await textResponse.text()).trim();
      if (bodyText) {
        return withStatus(bodyText);
      }
    } catch {
      // The response body may be unreadable; retain the status-based message.
    }

    return withStatus(fallbackMessage);
  }
}

function createFallbackSync(args: {
  sourceKey: string;
  shape: ShapeDefinition<unknown>;
  params: Record<string, string>;
  reportError: (error: SyncError) => void;
}) {
  return (syncParams: SyncParams): SyncResult => {
    let isCleanedUp = false;
    let refreshPromise: Promise<void> | null = null;

    const refreshNow = async () => {
      if (refreshPromise) {
        return refreshPromise;
      }

      refreshPromise = (async () => {
        try {
          const response = await makeRequest(
            buildFallbackRequestPath(args.shape.fallbackUrl, args.params),
            { method: 'GET', cache: 'no-store' }
          );

          if (!response.ok) {
            const message = await parseResponseError(
              response,
              `Failed to fetch fallback ${args.shape.table}`
            );
            throw new Error(message);
          }

          const payload = (await response.json()) as unknown;
          const rows = extractFallbackRows(payload, args.shape.table);
          fallbackSnapshotCache.set(args.sourceKey, rows);

          if (!isCleanedUp) {
            applySnapshot(syncParams, rows);
          }
        } catch (error) {
          if (isAbortError(error)) return;

          const message =
            error instanceof Error ? error.message : 'Fallback fetch failed';
          args.reportError({ message });

          if (!isCleanedUp && !syncParams.collection.isReady()) {
            syncParams.markReady();
          }
        } finally {
          refreshPromise = null;
        }
      })();

      return refreshPromise;
    };

    const unregisterRefresher = registerFallbackRefresher(
      args.sourceKey,
      async () => {
        // A request started before the mutation may return an older snapshot.
        // Wait for it, then fetch the committed state before retiring optimism.
        if (refreshPromise) await refreshPromise;
        await refreshNow();
      }
    );

    const cachedRows = fallbackSnapshotCache.get(args.sourceKey);
    if (cachedRows) {
      applySnapshot(syncParams, cachedRows);
    }

    void refreshNow();

    const intervalId = globalThis.setInterval(() => {
      void refreshNow();
    }, FALLBACK_REFRESH_INTERVAL_MS);

    return {
      cleanup: () => {
        isCleanedUp = true;
        globalThis.clearInterval(intervalId);
        unregisterRefresher();
      },
      loadSubset: () => true,
    };
  };
}

async function maybeRefreshFallbackAfterMutation(
  sourceKey: string
): Promise<void> {
  invalidateFallbackCache(sourceKey);
  await refreshFallbackSource(sourceKey);
}

export function refreshShapeFallback<TRow extends ElectricRow>(
  shape: ShapeDefinition<TRow>,
  params: Record<string, string>
): void {
  const sourceKey = buildSourceKey(shape.table, params);
  invalidateFallbackCache(sourceKey);
  void refreshFallbackSource(sourceKey);
}

function buildMutationHandlers(
  mutation: MutationDefinition<unknown, unknown, unknown>,
  sourceKey: string
) {
  return {
    onInsert: async ({ transaction }: MutationFnParams): Promise<void> => {
      await Promise.all(
        transaction.mutations.map(async (mutationItem) => {
          const data = mutationItem.modified as Record<string, unknown>;
          const response = await makeRequest(mutation.url, {
            method: 'POST',
            body: JSON.stringify(data),
          });

          if (!response.ok) {
            const message = await parseResponseError(
              response,
              `Failed to create ${mutation.name}`
            );
            throw new Error(message);
          }
        })
      );

      await maybeRefreshFallbackAfterMutation(sourceKey);
    },

    onUpdate: async ({ transaction }: MutationFnParams): Promise<void> => {
      if (transaction.mutations.length > 1) {
        const updates = transaction.mutations.map((mutationItem) => {
          if (!mutationItem.key) {
            throw new Error(`Failed to update ${mutation.name}: missing key`);
          }

          return {
            id: String(mutationItem.key),
            ...(mutationItem.changes as Record<string, unknown>),
          };
        });

        const response = await makeRequest(`${mutation.url}/bulk`, {
          method: 'POST',
          body: JSON.stringify({ updates }),
        });

        if (!response.ok) {
          const message = await parseResponseError(
            response,
            `Failed to bulk update ${mutation.name}`
          );
          throw new Error(message);
        }
      } else {
        const mutationItem = transaction.mutations[0];
        if (!mutationItem?.key) {
          throw new Error(`Failed to update ${mutation.name}: missing key`);
        }

        const response = await makeRequest(
          `${mutation.url}/${mutationItem.key}`,
          {
            method: 'PATCH',
            body: JSON.stringify(mutationItem.changes),
          }
        );

        if (!response.ok) {
          const message = await parseResponseError(
            response,
            `Failed to update ${mutation.name}`
          );
          throw new Error(message);
        }
      }

      await maybeRefreshFallbackAfterMutation(sourceKey);
    },

    onDelete: async ({ transaction }: MutationFnParams): Promise<void> => {
      await Promise.all(
        transaction.mutations.map(async (mutationItem) => {
          const response = await makeRequest(
            `${mutation.url}/${mutationItem.key}`,
            {
              method: 'DELETE',
            }
          );

          if (!response.ok) {
            const message = await parseResponseError(
              response,
              `Failed to delete ${mutation.name}`
            );
            throw new Error(message);
          }
        })
      );

      await maybeRefreshFallbackAfterMutation(sourceKey);
    },
  };
}

export function createShapeCollection<TRow extends ElectricRow>(
  shape: ShapeDefinition<TRow>,
  params: Record<string, string>,
  config?: CollectionConfig,
  mutation?: MutationDefinition<unknown, unknown, unknown>
) {
  const hasMutations = Boolean(mutation);
  const collectionId = buildCollectionId(shape.table, params, hasMutations);
  const sourceKey = buildSourceKey(shape.table, params);

  const cached = collectionCache.get(collectionId);
  if (cached) {
    return cached as typeof cached & { __rowType?: TRow };
  }

  const reportError = createErrorReporter(config);
  const collectionOptions = {
    id: collectionId,
    getKey: (item: ElectricRow) => getRowKey(item),
    gcTime: DEFAULT_GC_TIME_MS,
    ...(mutation ? buildMutationHandlers(mutation, sourceKey) : {}),
    sync: {
      sync: createFallbackSync({ sourceKey, shape, params, reportError }),
    },
  };

  const collection = createCollection(
    collectionOptions as never
  ) as unknown as ReturnType<typeof createCollection> & { __rowType?: TRow };

  collectionCache.set(collectionId, collection);
  return collection;
}
