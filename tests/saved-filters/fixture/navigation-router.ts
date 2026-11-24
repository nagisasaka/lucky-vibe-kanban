import {
  createRootRoute,
  createRoute,
  createRouter,
  createMemoryHistory,
} from "@tanstack/react-router";
import { projectSearchValidator } from "@/project-routes/project-search";
export const root = createRootRoute();
const app = createRoute({ getParentRoute: () => root, id: "_app" });
const routes = [
  "/projects/$projectId",
  "/projects/$projectId_/issues/$issueId",
  "/projects/$projectId_/issues/$issueId_/workspaces/$workspaceId",
  "/projects/$projectId_/workflows",
  "/workspaces",
].map((path) =>
  createRoute({
    getParentRoute: () => app,
    path: path.replaceAll("_/", "/"),
    validateSearch: projectSearchValidator,
  }).update({ id: path } as never),
);
export const router = createRouter({
  routeTree: root.addChildren([app.addChildren(routes)]),
  history: createMemoryHistory({ initialEntries: ["/projects/project"] }),
});
