import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { ConversationList } from "../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer";
const api = window as any;
api.source = (start: number, end: number, prefix = "a") => ({
  executionProcessState: {},
  liveExecutionProcesses: [],
  canonical: {
    entries: Array.from({ length: end - start }, (_, i) => ({
      type: "NORMALIZED_ENTRY",
      patchKey: `${prefix}-${i + start}`,
      content: {
        entry_type: { type: "user_message" },
        content: `${prefix}-${i + start}`,
        timestamp: null,
      },
    })),
    isRunning: false,
    activeAgentRunIds: new Set(),
  },
});
function Harness() {
  const [scope, setScope] = useState("a");
  api.setScope = setScope;
  return (
    <ConversationList attempt={{ id: scope, session: { id: scope } } as any} />
  );
}
createRoot(document.getElementById("root")!).render(<Harness />);
