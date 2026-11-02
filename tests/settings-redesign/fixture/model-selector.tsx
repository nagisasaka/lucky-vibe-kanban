import { useState } from "react";
import { ModelSelectorContainer } from "@/shared/components/ModelSelectorContainer";
import {
  BaseCodingAgent,
  ExecutionMode,
  PermissionPolicy,
  type ExecutorConfig,
} from "shared/types";
const params = new URLSearchParams(location.search);
const options = [
  { id: "raw-effort", label: "Raw reasoning 原文", is_default: true },
];
const models = [
  {
    id: "raw:model",
    name: "Raw Alpha 原文",
    provider_id: "alpha",
    reasoning_options: options,
  },
  {
    id: "raw:model",
    name: "Raw Beta 原文",
    provider_id: "beta",
    reasoning_options: options,
  },
];
const config = {
  models,
  providers: [
    { id: "alpha", name: "Raw provider A 原文" },
    { id: "beta", name: "Raw provider B 原文" },
  ],
  default_model: "alpha/raw:model",
  agents: [{ id: "raw-agent", label: "Raw agent 原文", is_default: true }],
  permissions: [
    PermissionPolicy.SUPERVISED,
    PermissionPolicy.AUTO,
    PermissionPolicy.PLAN,
  ],
};
export function useUserSystem() {
  return {
    profiles: null,
    setProfiles: () => {
      throw Error("Unexpected profile write");
    },
    reloadSystem: async () => {},
  };
}
export function useModelSelectorConfig() {
  return {
    config,
    loadingModels: false,
    error: params.has("model-error")
      ? "Raw provider secret /internal/path"
      : null,
  };
}
export function ModelSelectorFixture() {
  const [executorConfig, setConfig] = useState<ExecutorConfig>({
    model_id: "alpha/raw:model",
    reasoning_id: "raw-effort",
    execution_mode: ExecutionMode.code,
    permission_policy: PermissionPolicy.SUPERVISED,
  } as ExecutorConfig);
  const [events, setEvents] = useState<unknown[]>([]);
  return (
    <main className="min-h-screen bg-primary text-normal p-double">
      <div
        className="flex flex-wrap items-center gap-base"
        data-testid="model-controls"
      >
        <ModelSelectorContainer
          agent={BaseCodingAgent.CODEX}
          workspaceId="raw-workspace"
          sessionId="raw-session"
          presets={["DEFAULT", "raw-preset"]}
          selectedPreset="DEFAULT"
          onAdvancedSettings={() => {}}
          executorConfig={executorConfig}
          presetOptions={null}
          onOverrideChange={(patch) => {
            setEvents((old) => [...old, patch]);
            setConfig((old) => ({ ...old, ...patch }));
          }}
        />
      </div>
      <output data-testid="model-overrides">{JSON.stringify(events)}</output>
      <output data-testid="model-config">
        {JSON.stringify(executorConfig)}
      </output>
    </main>
  );
}
