import React, { useMemo, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  RouterProvider,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import {
  ProjectContext,
  type ProjectContextValue,
} from "@/shared/hooks/useProjectContext";
import { OrgContext, type OrgContextValue } from "@/shared/hooks/useOrgContext";
import { SettingsDirtyProvider } from "@/shared/dialogs/settings/settings/SettingsDirtyContext";
import { SavedFiltersManager } from "@/shared/components/saved-filters/SavedFiltersManager";
import { useSavedFilter } from "@/shared/hooks/useSavedFilter";
import { SavedFilterStatus } from "@/shared/components/saved-filters/SavedFilterStatus";
import NiceModal from "@ebay/nice-modal-react";
import { AppNavigationProvider } from "@/shared/hooks/useAppNavigation";
import type { AppNavigation } from "@/shared/lib/routes/appNavigation";
function Board() {
  const view = useSavedFilter();
  return (
    <section>
      <button
        onClick={() => view.select("11111111-1111-4111-8111-111111111111")}
      >
        Apply
      </button>
      <button
        onClick={() => view.select("22222222-2222-4222-8222-222222222222")}
      >
        Other
      </button>
      <button onClick={() => view.select(undefined)}>Clear</button>
      <button onClick={view.refresh}>Refresh</button>
      <div data-testid="selected">{view.id}</div>
      <div data-testid="result">
        {view.ready ? [...view.issueIds!].join(",") : "pending"}
      </div>
      <SavedFilterStatus view={view} count={view.issueIds?.size ?? 0} />
    </section>
  );
}
function Screen() {
  const [changed, setChanged] = useState(false);
  const project = useMemo(
    () =>
      ({
        projectId: "project",
        issues: [
          {
            id: "active",
            simple_id: "P-1",
            title: "Active",
            archived: false,
            updated_at: changed ? "new" : "old",
          },
          {
            id: "archived",
            simple_id: "P-2",
            title: "Archived",
            archived: true,
          },
        ],
        issueTags: [],
        issueAssignees: [],
        issueRelationships: [],
        tags: [],
        statuses: [],
        issuesById: new Map([
          ["active", {}],
          ["archived", {}],
        ]),
        isLoading: false,
      }) as unknown as ProjectContextValue,
    [changed],
  );
  return (
    <ProjectContext.Provider value={project}>
      <OrgContext.Provider
        value={{ membersWithProfilesById: new Map() } as OrgContextValue}
      >
        <div className="bg-primary text-normal p-double min-h-screen">
          <button onClick={() => setChanged(!changed)}>Change issue</button>
          <Board />
          <SettingsDirtyProvider>
            <SavedFiltersManager />
          </SettingsDirtyProvider>
        </div>
      </OrgContext.Provider>
    </ProjectContext.Provider>
  );
}
const root = createRootRoute({
  component: Screen,
  validateSearch: (search: Record<string, unknown>) => ({
    savedFilter:
      typeof search.savedFilter === "string" ? search.savedFilter : undefined,
  }),
});
const router = createRouter({ routeTree: root });
const client = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
export function Integration() {
  return (
    <QueryClientProvider client={client}>
      <NiceModal.Provider>
        <AppNavigationProvider
          value={
            {
              setProjectSavedFilter: (_projectId, savedFilter) => {
                void router.navigate({ to: "/", search: { savedFilter } });
              },
            } as AppNavigation
          }
        >
          <RouterProvider router={router} />
        </AppNavigationProvider>
      </NiceModal.Provider>
    </QueryClientProvider>
  );
}
