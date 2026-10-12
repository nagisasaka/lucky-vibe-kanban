import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ProjectChat } from "../../packages/web-core/src/features/kanban/ui/ProjectChat";
const client = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
window.astraClient = client;
window.astraView = {
  configured: true,
  project_state: {},
  messages: Array.from({ length: 100 }, (_, i) => ({
    id: String(i),
    sequence: i + 1,
    role: "assistant",
    status: "done",
    content: "History " + i,
    created_at: new Date().toISOString(),
  })),
};
window.astraRefresh = () =>
  client.invalidateQueries({ queryKey: ["project-chat", "test"] });
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={client}>
    <ProjectChat projectId="test" />
  </QueryClientProvider>,
);
