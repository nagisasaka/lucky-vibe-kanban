import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { GearIcon } from "@phosphor-icons/react";
import { Navbar } from "../../packages/ui/src/components/Navbar";
import { ResourceSummary } from "../../packages/ui/src/components/ResourceSummary";
import { useResourceObservation } from "../../packages/web-core/src/shared/hooks/useResourceObservation";

function DuplicateSubscriber() {
  useResourceObservation();
  return null;
}
function App() {
  const resources = useResourceObservation();
  return (
    <>
      <Navbar
        workspaceTitle="Example workspace"
        mobileMode={innerWidth < 600}
        isOnProjectPage={new URLSearchParams(location.search).has("project")}
        onReload={() => {}}
        onOpenSettings={() => {}}
        onOpenCommandBar={() => {}}
        rightItems={[{ id: "settings", icon: GearIcon, tooltip: "Settings" }]}
        resourceSlot={<ResourceSummary {...resources} />}
      />
      <DuplicateSubscriber />
    </>
  );
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient()}>
    <App />
  </QueryClientProvider>,
);
