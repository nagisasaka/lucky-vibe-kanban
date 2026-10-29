import { useState } from "react";
import { createRoot } from "react-dom/client";
import { ModelSelectorContainer } from "../../packages/web-core/src/shared/components/ModelSelectorContainer";
import { useExecutorConfig } from "../../packages/web-core/src/shared/hooks/useExecutorConfig";
import {
  BaseCodingAgent,
  PermissionPolicy,
  type ExecutorConfig,
} from "../../shared/types";

function Composer() {
  const [scratch, setScratch] = useState<ExecutorConfig | null>(() =>
    JSON.parse(localStorage.getItem("review-config") ?? "null"),
  );
  const { executorConfig, setOverrides } = useExecutorConfig({
    profiles: {
      CODEX: {
        DEFAULT: {
          CODEX: { append_prompt: null, plan: false, execution_mode: "code" },
        },
      },
    } as never,
    lastUsedConfig: {
      executor: BaseCodingAgent.CODEX,
      permission_policy: PermissionPolicy.SUPERVISED,
    },
    scratchConfig: scratch,
    onPersist: (config) => {
      const saved = JSON.parse(JSON.stringify(config));
      localStorage.setItem("review-config", JSON.stringify(saved));
      setScratch(saved);
    },
  });
  const [submitted, setSubmitted] = useState<ExecutorConfig | null>(null);
  return (
    <>
      <ModelSelectorContainer
        agent={BaseCodingAgent.CODEX}
        workspaceId={undefined}
        onAdvancedSettings={() => {}}
        presets={["DEFAULT"]}
        selectedPreset="DEFAULT"
        onPresetSelect={() => {}}
        onOverrideChange={setOverrides}
        executorConfig={executorConfig}
        presetOptions={null}
      />
      <output id="config">{JSON.stringify(executorConfig)}</output>
      <button onClick={() => setSubmitted(executorConfig)}>Submit</button>
      <output id="submitted">{JSON.stringify(submitted)}</output>
    </>
  );
}
function App() {
  const [key, setKey] = useState(0);
  return (
    <>
      <button onClick={() => setKey(key + 1)}>Remount composer</button>
      <Composer key={key} />
    </>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
