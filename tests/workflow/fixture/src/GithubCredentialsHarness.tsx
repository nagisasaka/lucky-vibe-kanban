import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMachineClient } from "@/shared/lib/machineClient";
import { GithubCredentialsSettings } from "@/shared/dialogs/settings/settings/GithubCredentialsSettings";
const query = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
const client = createMachineClient("local", {
  id: "local",
  apiHostId: null,
  kind: "local",
  label: "Local",
});
export function GithubCredentialsHarness() {
  return (
    <QueryClientProvider client={query}>
      <section style={{ maxWidth: 720, padding: 20 }}>
        <GithubCredentialsSettings repoId="test-repo" client={client} />
      </section>
    </QueryClientProvider>
  );
}
