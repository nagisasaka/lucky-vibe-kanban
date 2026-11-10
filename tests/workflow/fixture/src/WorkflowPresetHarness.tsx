import { useState } from "react";
import type { PresetRequest } from "shared/types";
import { WorkflowPresetFields } from "@/features/workflow/ui/WorkflowPresetFields";

export function WorkflowPresetHarness() {
  const [value, setValue] = useState<PresetRequest | null>(null);
  const [ready, setReady] = useState(false);
  return (
    <div style={{ maxWidth: 640, padding: 16 }}>
      <WorkflowPresetFields
        projectId="preset-fixture"
        value={value}
        onChange={setValue}
        onReady={setReady}
      />
      <button disabled={!ready}>Start fixture</button>
      <output data-testid="parameters">{JSON.stringify(value)}</output>
    </div>
  );
}
