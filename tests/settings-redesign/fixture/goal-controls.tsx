import { useState } from "react";
import { AgentGoalStatus, type AgentGoalState } from "../../../shared/types";
import { GoalProgressCard } from "@/features/workspace-chat/ui/GoalProgressCard";
export function GoalControlsFixture() {
  const [goal, setGoal] = useState<AgentGoalState>({
    objective: "Raw objective 日本語",
    status: AgentGoalStatus.active,
    tokens_used: 9007199254740993n,
    token_budget: 9999999999999999n,
    time_used_seconds: 3661n,
  });
  const [calls, setCalls] = useState<unknown[]>([]);
  const record = (action: string, value?: string) =>
    setCalls((previous) => [...previous, { action, value }]);
  return (
    <div className="p-base max-w-3xl bg-primary text-normal">
      <GoalProgressCard
        goal={goal}
        isRunActive
        onPause={() => {
          record("pause");
          setGoal((previous) => ({
            ...previous,
            status: AgentGoalStatus.paused,
          }));
        }}
        onResume={() => record("resume")}
        onEdit={(objective) => {
          record("edit", objective);
          setGoal((previous) => ({ ...previous, objective }));
        }}
        onClear={() => record("clear")}
        onInterruptTurn={() => record("stop-turn")}
      />
      <GoalProgressCard
        goal={{
          ...goal,
          objective: "Raw saved objective 原文",
          status: AgentGoalStatus.blocked,
        }}
        isRunActive={false}
        onPause={() => {}}
        onResume={() => {}}
        onEdit={() => {}}
        onClear={() => {}}
        onInterruptTurn={() => {}}
        onResumeSaved={() => record("resume-saved")}
      />
      <pre data-testid="goal-control-calls" className="overflow-x-auto">
        {JSON.stringify(calls)}
      </pre>
    </div>
  );
}
