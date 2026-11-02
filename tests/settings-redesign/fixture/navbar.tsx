import { useState } from "react";
import { Navbar } from "@vibe/ui/components/Navbar";
export function NavbarFixture() {
  const [tab, setTab] = useState("chat");
  const [events, setEvents] = useState<string[]>([]);
  const record = (event: string) => () => setEvents((old) => [...old, event]);
  return (
    <div className="bg-primary text-normal min-h-screen">
      <Navbar
        leftItems={[]}
        rightItems={[]}
        workspaceTitle="Raw workspace 原文"
        mobileMode
        mobileActiveTab={tab as "chat"}
        onMobileTabChange={setTab}
        onOpenDrawer={record("drawer")}
        onNavigateToBoard={record("board")}
        onOpenCommandBar={record("command")}
        onOpenSettings={record("settings")}
        onReload={record("reload")}
        syncErrors={[
          {
            streamId: "raw-stream",
            tableName: "raw-table",
            error: { message: "Raw internal secret", status: 503 },
          },
        ]}
      />
      <output data-testid="selected-tab">{tab}</output>
      <output data-testid="navbar-events">{JSON.stringify(events)}</output>
    </div>
  );
}
