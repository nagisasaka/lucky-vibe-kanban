import {
  WorkspaceWikiProvider,
  useWorkspaceWiki,
} from "@/features/wiki/ui/WorkspaceWikiProvider";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Repo } from "shared/types";
import { createMachineClient } from "@/shared/lib/machineClient";
import { RepositoryMemorySettings } from "@/shared/dialogs/settings/settings/RepositoryMemorySettings";
const query = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
const client = createMachineClient("local", {
  id: "local",
  apiHostId: null,
  kind: "local",
  label: "Local",
});
function PublishedWikiState() {
  const { snapshot } = useWorkspaceWiki();
  return (
    <span data-testid="canonical-wiki-state">
      {snapshot
        ? snapshot.wiki.exists
          ? "published"
          : "uninitialized"
        : "loading"}
    </span>
  );
}
export function RepositoryWikiHarness() {
  return (
    <QueryClientProvider client={query}>
      <WorkspaceWikiProvider
        repos={[]}
        projectId="wiki-project"
        mainRepos={[
          {
            id: "wiki-fixture",
            name: "Fixture",
            path: "/fixture",
            wiki_enabled: true,
          },
        ]}
      >
        <section className="mx-auto max-w-3xl p-double">
          <PublishedWikiState />
          <RepositoryMemorySettings
            repo={
              {
                id: "wiki-fixture",
                name: "fixture",
                display_name: "Fixture",
                default_target_branch: "main",
              } as Repo
            }
            client={client}
          />
        </section>
      </WorkspaceWikiProvider>
    </QueryClientProvider>
  );
}
