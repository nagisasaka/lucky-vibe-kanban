import { useState } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppNavigationProvider } from "@/shared/hooks/useAppNavigation";
import type { AppNavigation } from "@/shared/lib/routes/appNavigation";
import { ProjectChat } from "@/features/kanban/ui/ProjectChat";
import "@/i18n";
import "@/app/styles/new/index.css";

function Harness() {
  const [destination, setDestination] = useState("");
  const navigation = new Proxy(
    {},
    {
      get:
        (_, method) =>
        (...args: string[]) =>
          setDestination(`${String(method)}:${args.join(":")}`),
    },
  ) as AppNavigation;
  return (
    <AppNavigationProvider value={navigation}>
      <div className="new-design flex h-screen flex-col bg-primary text-normal p-base">
        <output aria-label="Navigation">{destination}</output>
        <ProjectChat projectId="10000000-0000-0000-0000-000000000001" />
      </div>
    </AppNavigationProvider>
  );
}

export function mount() {
  const root = createRoot(document.getElementById("fixture")!);
  root.render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <Harness />
    </QueryClientProvider>,
  );
}
