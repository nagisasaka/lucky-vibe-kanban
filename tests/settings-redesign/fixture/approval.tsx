import { useState } from "react";
import PendingApprovalEntry from "@/shared/components/NormalizedConversation/PendingApprovalEntry";
import { ApprovalFormProvider } from "@/shared/hooks/ApprovalForm";
import { TabNavContext } from "@/shared/hooks/TabNavigationContext";
import { useHotkeysContext } from "react-hotkeys-hook";
import WYSIWYGEditor from "@/shared/components/WYSIWYGEditor";
const params = new URLSearchParams(location.search);
const time = Date.now();
const info = {
  approval_id: "raw-approval",
  tool_name: "Raw tool 原文",
  execution_process_id: "raw-process",
  is_question: false,
  created_at: new Date(time - 60000).toISOString(),
  timeout_at: new Date(
    time + (params.has("approval-expired") ? -1000 : 600000),
  ).toISOString(),
};
export function useApprovals() {
  return { getPendingById: () => info };
}
export function ApprovalFixture() {
  const [tab, setTab] = useState<"logs" | "changes">("logs");
  const { activeScopes, enableScope, disableScope } = useHotkeysContext();
  const [events, setEvents] = useState<string[]>([]);
  return (
    <main className="min-h-screen bg-primary text-normal p-base">
      <button onClick={() => setTab(tab === "logs" ? "changes" : "logs")}>
        Toggle fixture tab
      </button>
      <button
        onClick={() =>
          activeScopes.includes("dialog")
            ? disableScope("dialog")
            : enableScope("dialog")
        }
      >
        Toggle fixture dialog scope
      </button>
      <output data-testid="approval-scopes">{activeScopes.join(",")}</output>
      <TabNavContext.Provider
        value={{
          activeTab: tab,
          setActiveTab: (value) => setTab(value as "logs" | "changes"),
        }}
      >
        <ApprovalFormProvider>
          <PendingApprovalEntry
            pendingStatus={{
              status: "pending_approval",
              approval_id: "raw-approval",
            }}
            executionProcessId={
              params.has("approval-missing") ? undefined : "raw-process"
            }
          >
            <pre>Raw tool 原文 / raw-command --flag</pre>
          </PendingApprovalEntry>
        </ApprovalFormProvider>
      </TabNavContext.Provider>
      <section data-testid="markdown-copy" className="mt-double">
        <WYSIWYGEditor
          disabled
          value={"Raw\\_value 原文"}
          onEdit={() => setEvents((old) => [...old, "edit"])}
          onDelete={() => setEvents((old) => [...old, "delete"])}
        />
      </section>
      <section data-testid="markdown-source" className="mt-double">
        <WYSIWYGEditor
          disabled
          preserveCopySource
          value={"Raw\\_source 原文"}
        />
      </section>
      <output data-testid="editor-events">{JSON.stringify(events)}</output>
    </main>
  );
}
