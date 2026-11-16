// Product settings component with fixture-only publication; no backend or GitHub.
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Repo, RepositoryPushPreview } from "shared/types";
import settings from "../../packages/web-core/src/i18n/locales/ja/settings.json";
import type { MachineClient } from "../../packages/web-core/src/shared/lib/machineClient";
import { ApiError } from "../../packages/web-core/src/shared/lib/api";
import { RepositoryPushSettings } from "../../packages/web-core/src/shared/dialogs/settings/settings/RepositoryPushSettings";
i18n
  .use(initReactI18next)
  .init({ lng: "ja", resources: { ja: { settings } }, initImmediate: false });
const state = {
  mode: "reject",
  pushes: 0,
  verifications: 0,
  preview: null as RepositoryPushPreview | null,
  request: "",
};
Object.assign(window, { pushFixture: state });
const client = {
  target: { id: "fixture" },
  queryScopeKey: ["machine", "fixture"],
  getRepoBranches: async () => [
    { name: "main", is_remote: false },
    { name: "dev", is_remote: false },
    { name: "origin/hidden", is_remote: true },
  ],
  listRepoRemotes: async () => [
    { name: "origin", url: "https://github.com/example/fixture.git" },
  ],
  previewRepositoryPush: async (
    _: string,
    selection: { branch: string; remote: string; project_id: string },
  ) => {
    state.preview = {
      repository_id: "repo",
      repository_name: "Fixture repository",
      project_id: selection.project_id,
      remote: selection.remote,
      remote_url: "https://github.com/example/fixture.git",
      branch: selection.branch,
      destination_branch: selection.branch,
      source_sha: "a".repeat(40),
      remote_sha: "b".repeat(40),
      unpublished_commits: 2,
      issue_archive_paths: ["docs/changes/2026-11-17-LOCAL-385-fixture.md"],
    };
    return state.preview;
  },
  pushRepository: async (
    _: string,
    request: { request_id: string; preview: RepositoryPushPreview },
  ) => {
    state.pushes++;
    state.request = request.request_id;
    if (state.mode === "reject")
      throw new ApiError("API request failed", 200, undefined, {
        not_admitted: true,
        outcome: "source_drift",
      });
    throw new Error("Fixture response lost; verify the original request");
  },
  verifyRepositoryPush: async (_: string, request: string) => {
    state.verifications++;
    return {
      receipt: {
        request_id: request,
        status:
          state.mode === "verify-fail"
            ? "failed"
            : state.mode === "verify-uncertain"
              ? "uncertain"
              : "succeeded",
        result: {
          outcome: state.mode === "verify-fail" ? "protected_branch" : "pushed",
        },
        preview: state.preview ?? {
          repository_id: "repo",
          repository_name: "Fixture repository",
          project_id: "project",
          remote: "origin",
          remote_url: "https://github.com/example/fixture.git",
          branch: "dev",
          destination_branch: "dev",
          source_sha: "a".repeat(40),
          remote_sha: "b".repeat(40),
          unpublished_commits: 2,
          issue_archive_paths: [],
        },
      },
      remote_sha: state.mode === "verify-fail" ? null : "a".repeat(40),
      source_present: state.mode === "verify-fail" ? null : true,
      verification_error:
        state.mode === "verify-fail" ? "authentication_rejected" : null,
    };
  },
} as unknown as MachineClient;
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient()}>
    <main className="mx-auto max-w-2xl p-double">
      <RepositoryPushSettings
        repo={
          {
            id: "repo",
            display_name: "Fixture repository",
            path: "/fixture/registered",
          } as Repo
        }
        projectId="project"
        projects={[]}
        client={client}
      />
    </main>
  </QueryClientProvider>,
);
