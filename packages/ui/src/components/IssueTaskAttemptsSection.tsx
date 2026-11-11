import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  CollapsibleSectionHeader,
  type SectionAction,
} from "./CollapsibleSectionHeader";
import {
  IssueTaskAttemptCard,
  type IssueTaskAttemptCardData,
} from "./IssueTaskAttemptCard";

export interface IssueTaskAttemptsSectionProps {
  attempts: IssueTaskAttemptCardData[];
  arenaOption?: ReactNode;
  isLoading?: boolean;
  createDisabledReason?: string;
  actions?: SectionAction[];
  onOpenAttempt?: (attempt: IssueTaskAttemptCardData) => void;
  onRunAttempt?: (attempt: IssueTaskAttemptCardData) => void;
  onUnlinkAttempt?: (attempt: IssueTaskAttemptCardData) => void;
  onDeleteAttempt?: (attempt: IssueTaskAttemptCardData) => void;
  onStartAttempt?: () => void;
  onCreateWorkflowAttempt?: () => void;
  onCreateSingleAgentAttempt?: () => void;
}

export function IssueTaskAttemptsSection({
  attempts,
  arenaOption,
  isLoading,
  createDisabledReason,
  actions = [],
  onOpenAttempt,
  onRunAttempt,
  onUnlinkAttempt,
  onDeleteAttempt,
  onStartAttempt,
  onCreateWorkflowAttempt,
  onCreateSingleAgentAttempt,
}: IssueTaskAttemptsSectionProps) {
  const { t } = useTranslation("common");
  const hasCreateActions =
    !!onStartAttempt ||
    !!onCreateWorkflowAttempt ||
    !!onCreateSingleAgentAttempt ||
    !!arenaOption;

  return (
    <CollapsibleSectionHeader
      className="issue-detail-section"
      title={t("attempts.title", "Task Attempts")}
      persistKey="kanban-issue-task-attempts"
      defaultExpanded={true}
      actions={actions}
    >
      {createDisabledReason && (
        <p className="text-low px-base">{createDisabledReason}</p>
      )}
      <div className="flex flex-col gap-base border-t issue-section-inset p-base px-base">
        {isLoading ? (
          <p className="py-half text-low">
            {t("attempts.loading", "Loading attempts...")}
          </p>
        ) : (
          <>
            {hasCreateActions && (
              <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,16rem),1fr))] gap-base">
                {onStartAttempt && (
                  <IssueAttemptOption
                    title={t("workflow.presets.start", "Start work")}
                    description={t(
                      "workflow.presets.startDescription",
                      "Single agent or a fixed Workflow preset",
                    )}
                    disabledReason={createDisabledReason}
                    onClick={onStartAttempt}
                  />
                )}
                {onCreateWorkflowAttempt && (
                  <IssueAttemptOption
                    title={t("attempts.newWorkflow")}
                    description={t("attempts.workflowDescription")}
                    disabledReason={createDisabledReason}
                    onClick={onCreateWorkflowAttempt}
                  />
                )}
                {onCreateSingleAgentAttempt && (
                  <IssueAttemptOption
                    title={t("attempts.newSingleAgent")}
                    description={t("attempts.singleAgentDescription")}
                    disabledReason={createDisabledReason}
                    onClick={onCreateSingleAgentAttempt}
                  />
                )}
                {arenaOption}
              </div>
            )}
            {attempts.length === 0 ? (
              <p className="py-half text-xs text-low">
                {t("attempts.empty", "No task attempts yet.")}
              </p>
            ) : (
              attempts.map((attempt) => (
                <IssueTaskAttemptCard
                  key={attempt.id}
                  attempt={attempt}
                  onOpen={() => onOpenAttempt?.(attempt)}
                  onRun={
                    attempt.kind === "workflow" && onRunAttempt
                      ? () => onRunAttempt(attempt)
                      : undefined
                  }
                  onUnlink={
                    attempt.kind === "single_agent" && onUnlinkAttempt
                      ? () => onUnlinkAttempt(attempt)
                      : undefined
                  }
                  onDelete={
                    onDeleteAttempt ? () => onDeleteAttempt(attempt) : undefined
                  }
                />
              ))
            )}
          </>
        )}
      </div>
    </CollapsibleSectionHeader>
  );
}

/** The three ways of approaching an Issue share one visual and interaction level. */
export function IssueAttemptOption({
  title,
  description,
  disabledReason,
  onClick,
}: {
  title: string;
  description: string;
  disabledReason?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={!!disabledReason}
      title={disabledReason}
      onClick={onClick}
      className="flex min-w-0 flex-col gap-half rounded-sm border border-border bg-secondary/40 p-plusfifty text-left transition-colors hover:bg-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50 disabled:cursor-not-allowed"
    >
      <span className="text-sm font-medium text-high">{title}</span>
      <span className="text-sm leading-relaxed text-low">{description}</span>
    </button>
  );
}
