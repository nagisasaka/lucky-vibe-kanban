import { KeyValueField } from "@/shared/dialogs/settings/settings/rjsf/Fields";
import type { FieldProps } from "../../../packages/web-core/node_modules/@rjsf/utils";
import { useMemo, useState } from "react";
import { CreateConfigurationDialog } from "@/shared/dialogs/settings/CreateConfigurationDialog";
import { DeleteConfigurationDialog } from "@/shared/dialogs/settings/DeleteConfigurationDialog";
import { RepositoryMemorySettings } from "@/shared/dialogs/settings/settings/RepositoryMemorySettings";
import type { MachineClient } from "@/shared/lib/machineClient";
import type { Repo, RepositoryMemoryState } from "../../../shared/types";

// Deliberately supplies in-memory methods only. No native/auth/Wiki writes exist.
export function RepositoryFormsFixture() {
  const [environment, setEnvironment] = useState<
    Record<string, string> | undefined
  >({ RAW_KEY: "Raw value 原文" });
  const [result, setResult] = useState<unknown>(null);
  const params = new URLSearchParams(location.search);
  const repo: Repo = {
    id: "repository-fixture",
    path: "/fixture",
    name: "Raw repo 原文",
    display_name: "Raw repo 原文",
    setup_script: null,
    cleanup_script: null,
    archive_script: null,
    copy_files: null,
    parallel_setup_script: false,
    dev_server_script: null,
    default_target_branch: "main",
    default_working_dir: null,
    created_at: new Date(0),
    updated_at: new Date(0),
  };
  const client = useMemo(() => {
    const calls: Array<{ action: string; data?: unknown }> = [];
    Object.defineProperty(window, "__repositoryFormsCalls", { value: calls });
    let memory: RepositoryMemoryState = {
      version: 1,
      enabled: true,
      status: params.get("memoryError") === "true" ? "error" : "current",
      target_branch: "main",
      source_commit: "a".repeat(40),
      wiki_commit: "b".repeat(40),
      last_success: "2026-01-02T15:04:05Z",
      active_run_id:
        params.get("memoryActive") === "true" ? "run-fixture" : null,
      bootstrap: null,
      maintenance_workspace_id: null,
      maintenance_session_id: null,
      error:
        params.get("memoryError") === "true"
          ? "fixture private maintenance detail"
          : null,
      output_language: "en",
      active_source_commit: null,
      active_event_ids: [],
      active_sync_input_digest: null,
      coding_errors:
        params.get("memoryError") === "true"
          ? [
              "Workspace 11111111-1111-1111-1111-111111111111 / run 22222222-2222-2222-2222-222222222222: fixture private checkpoint detail",
            ]
          : [],
      wiki_exists: true,
      active_sync_scope: { type: "current_source" },
      unverified_before: null,
    };
    const methods: Pick<
      MachineClient,
      | "queryScopeKey"
      | "target"
      | "getRepositoryMemory"
      | "configureRepositoryMemory"
      | "syncRepositoryMemory"
    > = {
      queryScopeKey: ["machine", "repository-form-fixture"],
      target: {
        kind: "local",
        id: "local",
        apiHostId: null,
        label: "Local fixture",
      },
      getRepositoryMemory: async () => structuredClone(memory),
      configureRepositoryMemory: async (_repo, input) => {
        calls.push({ action: "configureMemory", data: input });
        memory = { ...memory, ...input };
        return structuredClone(memory);
      },
      syncRepositoryMemory: async () => {
        calls.push({ action: "syncMemory" });
        throw new Error("Fixture forbids Wiki maintenance");
      },
    };
    return methods as MachineClient;
  }, []);
  return (
    <div className="max-w-3xl space-y-double p-double text-normal bg-primary">
      <button
        onClick={() =>
          void CreateConfigurationDialog.show({
            executorType: "CODEX",
            existingConfigs: ["DEFAULT"],
          }).then(setResult)
        }
      >
        Fixture create configuration
      </button>
      <button
        onClick={() =>
          void DeleteConfigurationDialog.show({
            executorType: "CODEX",
            configName: "CUSTOM_RAW",
          }).then(setResult)
        }
      >
        Fixture delete configuration
      </button>
      <pre data-testid="configuration-result">{JSON.stringify(result)}</pre>
      <section data-testid="environment-field">
        <KeyValueField
          {...({
            formData: environment,
            disabled: false,
            readonly: false,
            registry: { formContext: { onEnvChange: setEnvironment } },
          } as FieldProps<Record<string, string>>)}
        />
        <pre data-testid="environment-value">{JSON.stringify(environment)}</pre>
      </section>
      <section data-testid="repository-memory">
        <RepositoryMemorySettings repo={repo} client={client} />
      </section>
    </div>
  );
}
