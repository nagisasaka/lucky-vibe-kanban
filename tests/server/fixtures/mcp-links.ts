// Exercise the real local router and navigation resolver without mounting app
// providers or accessing an LVK database. This fixture does not simulate UI data.
import { router } from "../../../packages/local-web/src/app/router";
import { localAppNavigation } from "../../../packages/local-web/src/app/navigation/AppNavigation";

await router.load();
const current = router.state.location;
const destination = localAppNavigation.resolveFromPath(current.pathname);
const output = document.createElement("pre");
output.id = "route-result";
output.textContent = JSON.stringify({
  pathname: current.pathname,
  publicHref: current.publicHref,
  destination,
  matches: router.state.matches.map((match) => ({
    id: match.routeId,
    params: match.params,
    status: match.status,
  })),
});
document.body.append(output);
if (destination?.kind === "project-issue-workspace") {
  const link = document.createElement("a");
  link.id = "issue-link";
  link.textContent = "Open Issue";
  link.href = router.buildLocation({
    to: "/projects/$projectId/issues/$issueId",
    params: { projectId: destination.projectId, issueId: destination.issueId },
  }).publicHref;
  document.body.append(link);
}
