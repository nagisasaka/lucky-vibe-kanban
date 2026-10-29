// Production landing/layout/sidebar/navbar with fixture data and inert detail tools.
import { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import common from "../../packages/web-core/src/i18n/locales/en/common.json";
import { WorkspacesLanding } from "../../packages/web-core/src/pages/workspaces/WorkspacesLanding";
import { Workspaces } from "../../packages/web-core/src/pages/workspaces/Workspaces";
import { WorkspacesSidebar } from "../../packages/ui/src/components/WorkspacesSidebar";
import { MobileDrawer } from "../../packages/ui/src/components/MobileDrawer";
import { Navbar, MOBILE_TABS } from "../../packages/ui/src/components/Navbar";
import { useMobileActiveTab } from "../../packages/web-core/src/shared/stores/useUiPreferencesStore";
import { useIsMobile } from "../../packages/web-core/src/shared/hooks/useIsMobile";

void i18n
  .use(initReactI18next)
  .init({ lng: "en", resources: { en: { common } }, defaultNS: "common" });
let route: (id?: string) => void = () => {};
let selected: string | undefined;
let empty = false;
export function context() {
  return {
    workspaceId: selected,
    workspace: selected ? { id: selected, name: selected } : undefined,
    isCreateMode: false,
    isLoading: false,
    repos: [],
    sessions: [],
    isSessionsLoading: false,
  };
}
export function Sidebar() {
  const [, setTab] = useMobileActiveTab();
  return (
    <WorkspacesSidebar
      layoutMode="accordion"
      workspaces={
        empty
          ? []
          : Array.from({ length: 40 }, (_, i) => ({
              id: `fixture-${i}`,
              name: `Workspace ${i}`,
              updatedAt: "2026-10-29T00:00:00Z",
              createdAt: "2026-10-29T00:00:00Z",
            }))
      }
      totalWorkspacesCount={empty ? 0 : 40}
      selectedWorkspaceId={selected ?? null}
      searchQuery=""
      onSearchChange={() => {}}
      onSelectWorkspace={(id) => {
        setTab("chat");
        route(id);
      }}
    />
  );
}
function App() {
  const [id, setId] = useState<string>();
  const [, refresh] = useState(0);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [tab, setTab] = useMobileActiveTab();
  const mobile = useIsMobile();
  route = (next?: string) => {
    selected = next;
    history.pushState({}, "", next ? `/workspaces/${next}` : "/workspaces");
    setId(next);
  };
  return (
    <div className="h-full flex flex-col">
      {!mobile && <button onClick={() => route()}>Local workspaces</button>}
      <MobileDrawer
        open={mobile && drawerOpen}
        onClose={() => setDrawerOpen(false)}
      >
        <button
          onClick={() => {
            route();
            setDrawerOpen(false);
          }}
        >
          Workspaces
        </button>
      </MobileDrawer>
      <button
        onClick={() => {
          empty = !empty;
          refresh((n) => n + 1);
          route();
        }}
      >
        Toggle empty fixture
      </button>
      <Navbar
        onOpenDrawer={() => setDrawerOpen(true)}
        mobileMode={mobile}
        mobileTabs={MOBILE_TABS}
        mobileActiveTab={tab}
        onMobileTabChange={setTab}
      />
      <div className="flex-1 min-h-0">
        {id ? <Workspaces /> : <WorkspacesLanding />}
      </div>
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
