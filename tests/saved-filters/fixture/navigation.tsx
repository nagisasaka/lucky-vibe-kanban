import React from "react";
import { RouterProvider, useRouterState } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";
import { router, root } from "./navigation-router";
import { localAppNavigation } from "../../../packages/local-web/src/app/navigation/AppNavigation";
const run = "33333333-3333-4333-8333-333333333333";
function App() {
  const location = useRouterState({ select: (state) => state.location });
  return (
    <>
      <button
        onClick={() =>
          void router.navigate({
            to: "/projects/$projectId/issues/$issueId/workspaces/$workspaceId",
            params: { projectId: "project", issueId: "one", workspaceId: "ws" },
            search: {
              savedFilter: "filter",
              session_id: "session",
              agent_run_id: run,
            },
          })
        }
      >
        Seed workspace
      </button>
      <button
        onClick={() =>
          void router.navigate({
            to: "/projects/$projectId/issues/$issueId/workspaces/$workspaceId",
            params: { projectId: "project", issueId: "one", workspaceId: "ws" },
            search: { session_id: "session", agent_run_id: run },
          })
        }
      >
        Seed ordinary
      </button>
      <button
        onClick={() =>
          void router.navigate({
            to: "/projects/$projectId/issues/$issueId",
            params: { projectId: "project", issueId: "two" },
          })
        }
      >
        Baseline issue
      </button>
      <button
        onClick={() => localAppNavigation.goToProjectIssue("project", "two")}
      >
        Issue
      </button>
      <button onClick={() => localAppNavigation.goToProject("project")}>
        Board
      </button>
      <button onClick={() => localAppNavigation.goToProject("other")}>
        Other project
      </button>
      <button onClick={() => localAppNavigation.goToWorkspaces()}>
        Workspaces
      </button>
      <output data-testid="location">
        {JSON.stringify({
          pathname: location.pathname,
          search: location.search,
        })}
      </output>
    </>
  );
}
root.update({ component: App });
createRoot(document.getElementById("root")!).render(
  <RouterProvider router={router} />,
);
