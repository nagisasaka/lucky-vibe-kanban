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
        mobileMode={innerWidth < 768}
        isOnProjectPage={new URLSearchParams(location.search).has("project")}
        onReload={() => {}}
        onOpenSettings={() => {}}
        onOpenCommandBar={() => {}}
        rightItems={["Integration", "Command Bar", "Guide", "Settings"].map(
          (label) => ({ id: label, icon: GearIcon, tooltip: label }),
        )}
        mobileUserSlot={
          <button aria-label="User settings" className="size-10 shrink-0">
            <GearIcon />
          </button>
        }
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
