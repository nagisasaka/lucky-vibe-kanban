import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HotkeysProvider } from "react-hotkeys-hook";
import { IntegrationPanel } from "../../../packages/web-core/src/features/kanban/ui/IntegrationPanel";
import "../../../packages/web-core/src/app/styles/new/index.css";
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <HotkeysProvider>
      <IntegrationPanel focusedRunId={null} onClearFocus={() => {}} />
    </HotkeysProvider>
  </QueryClientProvider>,
);
