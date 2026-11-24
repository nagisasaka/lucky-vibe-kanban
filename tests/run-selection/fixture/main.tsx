// Real picker + override hook; only provider discovery and saved profile are fixtures.
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import common from "@/i18n/locales/ja/common.json";
import { ModelSelectorContainer } from "@/shared/components/ModelSelectorContainer";
import { UserSystemContext } from "@/shared/hooks/useUserSystem";
import { useExecutorConfig } from "@/shared/hooks/useExecutorConfig";
import { presetOptionsKeys } from "@/shared/hooks/usePresetOptions";
import { BaseCodingAgent, PermissionPolicy } from "shared/types";
import "@/app/styles/new/index.css";

await i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { common } },
  defaultNS: "common",
  interpolation: { escapeValue: false },
});
const params = new URLSearchParams(location.search);
const scenario = params.get("scenario") ?? "configured";
const agent = scenario.includes("claude")
  ? BaseCodingAgent.CLAUDE_CODE
  : BaseCodingAgent.CODEX;
const preset =
  scenario === "profile"
    ? {
        executor: agent,
        model_id: "alpha",
        reasoning_id: "high",
        permission_policy: PermissionPolicy.ON_REQUEST,
      }
    : { executor: agent };
const profiles = { [agent]: { DEFAULT: { [agent]: {} } } };
const client = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
});
client.setQueryData(presetOptionsKeys.byProfile(agent, "DEFAULT"), preset);
const system = { profiles, setProfiles() {}, reloadSystem: async () => {} };
const unknown = scenario.includes("failed") || scenario === "empty";
const configuredEffort = ["configured", "claude-configured"].includes(scenario);
const defaults = {
  model: unknown ? null : "alpha",
  reasoning:
    unknown || scenario.includes("partial")
      ? null
      : scenario === "profile"
        ? "medium"
        : "high",
  approval: unknown ? null : "never",
  model_source: scenario.includes("default") ? "provider" : "configured",
  reasoning_source: configuredEffort ? "configured" : "provider",
};
const models = [
  {
    id: "alpha",
    name: "Alpha",
    reasoning_options: [
      {
        id: "medium",
        label: "Medium",
        is_default:
          unknown || scenario.includes("partial") || scenario === "profile",
      },
      {
        id: "high",
        label: "High",
        is_default:
          !unknown && !scenario.includes("partial") && scenario !== "profile",
      },
    ],
  },
  {
    id: "beta",
    name: "Beta",
    reasoning_options: [
      { id: "low", label: "Low", is_default: !configuredEffort },
      { id: "high", label: "High", is_default: configuredEffort },
    ],
  },
  { id: "haiku", name: "Haiku", reasoning_options: [] },
];
if (scenario === "claude-demand") {
  models[1].reasoning_options.forEach((r) => {
    r.is_default = false;
  });
  models.push({
    id: "gamma",
    name: "Gamma",
    reasoning_options: [{ id: "medium", label: "Medium", is_default: false }],
  });
}
const discovery = {
  scope: {
    agent,
    hostId: null,
    variant: "DEFAULT",
    workspaceId: "fixture",
    sessionId: scenario === "resume" ? "session" : undefined,
  },
  options: {
    model_selector: {
      models,
      providers: [],
      agents: [],
      permissions:
        agent === BaseCodingAgent.CODEX
          ? [
              PermissionPolicy.AUTO,
              PermissionPolicy.ON_REQUEST,
              PermissionPolicy.SUPERVISED,
            ]
          : [PermissionPolicy.AUTO, PermissionPolicy.SUPERVISED],
      run_defaults: defaults,
      unsupported_reasoning_models: ["haiku"],
    },
    loading_models: false,
  },
  error: null,
  isConnected: true,
  isInitialized: true,
};
function Fixture() {
  const [scratch, setScratch] = useState(null);
  const [sent, setSent] = useState(null);
  const [phase, setPhase] = useState(0);
  const [lastUsed, setLastUsed] = useState(null);
  const sessionId = phase > 0 || scenario === "resume" ? "session" : undefined;
  const currentDiscovery = {
    ...discovery,
    scope: { ...discovery.scope, sessionId },
  };
  const { executorConfig, presetOptions, setOverrides } = useExecutorConfig({
    profiles,
    lastUsedConfig: lastUsed,
    scratchConfig: scratch,
    configExecutorProfile: { executor: agent, variant: "DEFAULT" },
    onPersist: setScratch,
  });
  return (
    <UserSystemContext.Provider value={system as never}>
      <main className="bg-primary text-normal p-double">
        <div className="flex flex-wrap gap-base" data-testid="picker">
          <ModelSelectorContainer
            agent={agent}
            workspaceId="fixture"
            sessionId={sessionId}
            discovery={currentDiscovery as never}
            presets={["DEFAULT"]}
            selectedPreset="DEFAULT"
            onPresetSelect={() => {}}
            onAdvancedSettings={() => {}}
            executorConfig={executorConfig}
            presetOptions={presetOptions}
            onOverrideChange={setOverrides}
          />
        </div>
        <button onClick={() => setSent(executorConfig)} className="p-base">
          送信値を確認
        </button>
        <button
          onClick={() => {
            setLastUsed(executorConfig);
            setPhase(phase + 1);
          }}
        >
          Run完了
        </button>
        <output data-testid="phase">{phase}</output>
        <pre className="whitespace-pre-wrap break-all" data-testid="draft">
          {JSON.stringify(executorConfig)}
        </pre>
        <pre className="whitespace-pre-wrap break-all" data-testid="sent">
          {JSON.stringify(sent)}
        </pre>
      </main>
    </UserSystemContext.Provider>
  );
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={client}>
    <Fixture />
  </QueryClientProvider>,
);
