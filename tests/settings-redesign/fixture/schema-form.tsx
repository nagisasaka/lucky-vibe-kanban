import { useState } from "react";
import { BaseCodingAgent } from "../../../shared/types";
import { ExecutorConfigForm } from "@/shared/dialogs/settings/settings/ExecutorConfigForm";
const initial = { model: "Raw model 原文", goal_token_budget: 2 };
export function SchemaFormFixture() {
  const [observed, setObserved] = useState<unknown>(initial);
  return (
    <div className="p-6 max-w-3xl text-normal bg-primary">
      <ExecutorConfigForm
        executor={BaseCodingAgent.CODEX}
        value={initial}
        onChange={setObserved}
        onSave={async () => {}}
        isDirty
      />
      <pre className="overflow-x-auto" data-testid="schema-raw-values">
        {JSON.stringify(observed)}
      </pre>
    </div>
  );
}
