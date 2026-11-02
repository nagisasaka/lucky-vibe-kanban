import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useEffect, useState } from 'react';
import { AgentGoalStatus, type AgentGoalState } from 'shared/types';
import { watchGoalElapsed } from '../model/goalElapsed';

interface GoalProgressCardProps {
  goal: AgentGoalState;
  isRunActive: boolean;
  busy?: boolean;
  controllable?: boolean;
  error?: string | null;
  onPause: () => void;
  onResume: () => void;
  onResumeSaved?: () => void;
  resumeSavedDisabled?: boolean;
  onEdit: (objective: string) => void;
  onClear: () => void;
  onInterruptTurn: () => void;
}

function formatDuration(t: TFunction<'tasks'>, totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return hours > 0
    ? t('tasks:workspaceGoal.durationHours', { hours, minutes, seconds })
    : t('tasks:workspaceGoal.durationMinutes', { minutes, seconds });
}

export function GoalProgressCard({
  goal,
  isRunActive,
  busy = false,
  controllable = true,
  error,
  onPause,
  onResume,
  onResumeSaved,
  resumeSavedDisabled = false,
  onEdit,
  onClear,
  onInterruptTurn,
}: GoalProgressCardProps) {
  const { t, i18n } = useTranslation('tasks');
  const statusLabels: Record<AgentGoalStatus, string> = {
    [AgentGoalStatus.active]: t('workspaceGoal.active'),
    [AgentGoalStatus.paused]: t('workspaceGoal.paused'),
    [AgentGoalStatus.blocked]: t('workspaceGoal.blocked'),
    [AgentGoalStatus.usageLimited]: t('workspaceGoal.usageLimited'),
    [AgentGoalStatus.budgetLimited]: t('workspaceGoal.budgetLimited'),
    [AgentGoalStatus.complete]: t('workspaceGoal.complete'),
  };

  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(goal.objective);
  const [elapsed, setElapsed] = useState(Number(goal.time_used_seconds));

  useEffect(() => {
    setDraft(goal.objective);
  }, [goal.objective]);

  useEffect(() => {
    return watchGoalElapsed(
      Number(goal.time_used_seconds),
      goal.status === AgentGoalStatus.active,
      isRunActive,
      setElapsed
    );
  }, [goal.status, goal.time_used_seconds, isRunActive]);

  const canResume = [
    AgentGoalStatus.paused,
    AgentGoalStatus.blocked,
    AgentGoalStatus.usageLimited,
    AgentGoalStatus.budgetLimited,
  ].includes(goal.status);
  const buttonClass =
    'border-border hover:bg-secondary rounded-sm border px-base py-half text-xs text-normal disabled:cursor-not-allowed disabled:opacity-50';

  return (
    <section className="border-border bg-primary mb-base rounded-md border p-base">
      <div className="flex flex-wrap items-center justify-between gap-base">
        <div className="flex items-center gap-base">
          <span className="text-sm font-medium text-high">
            {isRunActive
              ? t('workspaceGoal.goal')
              : t('workspaceGoal.savedGoal')}
          </span>
          <span className="bg-secondary rounded-full px-base py-half text-xs text-normal">
            {isRunActive
              ? statusLabels[goal.status]
              : t('workspaceGoal.noActive')}
          </span>
        </div>
        <div className="flex flex-wrap gap-base text-xs text-low">
          <span>
            {t('workspaceGoal.tokens', {
              used: new Intl.NumberFormat(
                i18n.resolvedLanguage || i18n.language
              ).format(goal.tokens_used),
              budget:
                goal.token_budget == null
                  ? t('workspaceGoal.auto')
                  : new Intl.NumberFormat(
                      i18n.resolvedLanguage || i18n.language
                    ).format(goal.token_budget),
            })}
          </span>
          <span>
            {isRunActive
              ? t('workspaceGoal.elapsed')
              : t('workspaceGoal.recordedTime')}
            :{' '}
            {formatDuration(
              t,
              isRunActive ? elapsed : Number(goal.time_used_seconds)
            )}
          </span>
        </div>
      </div>

      {!isRunActive && (
        <p className="mt-base text-xs text-low">
          {t('workspaceGoal.savedHelp', { status: statusLabels[goal.status] })}
        </p>
      )}

      {isEditing && isRunActive && controllable ? (
        <div className="mt-base space-y-base">
          <textarea
            aria-label={t('workspaceGoal.objectiveLabel')}
            value={draft}
            onChange={(event) => setDraft(event.currentTarget.value)}
            rows={5}
            className="border-border bg-secondary text-normal w-full resize-y rounded-sm border p-base text-sm"
          />
          <p className="text-xs text-warning">
            {t('workspaceGoal.resetWarning')}
          </p>
          <div className="flex justify-end gap-base">
            <button
              type="button"
              className={buttonClass}
              onClick={() => {
                setDraft(goal.objective);
                setIsEditing(false);
              }}
            >
              {t('workspaceGoal.cancel')}
            </button>
            <button
              type="button"
              className={buttonClass}
              disabled={busy || !draft.trim() || draft === goal.objective}
              onClick={() => {
                onEdit(draft.trim());
                setIsEditing(false);
              }}
            >
              {t('workspaceGoal.save')}
            </button>
          </div>
        </div>
      ) : (
        <p className="mt-base max-h-24 overflow-y-auto whitespace-pre-wrap text-sm text-normal">
          {goal.objective}
        </p>
      )}

      {!isEditing && controllable && isRunActive && (
        <div className="mt-base flex flex-wrap gap-base">
          {goal.status === AgentGoalStatus.active && (
            <button
              type="button"
              className={buttonClass}
              disabled={busy}
              onClick={onPause}
            >
              {t('workspaceGoal.pause')}
            </button>
          )}
          {canResume && (
            <button
              type="button"
              className={buttonClass}
              disabled={busy}
              onClick={onResume}
            >
              {t('workspaceGoal.resume')}
            </button>
          )}
          <button
            type="button"
            className={buttonClass}
            disabled={busy}
            onClick={() => setIsEditing(true)}
          >
            {t('workspaceGoal.edit')}
          </button>
          <button
            type="button"
            className={buttonClass}
            disabled={busy}
            onClick={onInterruptTurn}
          >
            {t('workspaceGoal.stopTurn')}
          </button>
          <button
            type="button"
            className={buttonClass}
            disabled={busy}
            onClick={() => {
              if (window.confirm(t('workspaceGoal.clearConfirm'))) onClear();
            }}
          >
            {t('workspaceGoal.clear')}
          </button>
        </div>
      )}
      {!isRunActive &&
        goal.status !== AgentGoalStatus.complete &&
        onResumeSaved && (
          <div className="mt-base space-y-base">
            <button
              type="button"
              className={buttonClass}
              disabled={busy || resumeSavedDisabled}
              onClick={onResumeSaved}
            >
              {t('workspaceGoal.resumeSaved')}
            </button>
            <p className="text-xs text-low">{t('workspaceGoal.resumeHelp')}</p>
          </div>
        )}
      {error && (
        <p className="mt-base text-xs text-error">
          {t('workspaceGoal.error')} {error}
        </p>
      )}
    </section>
  );
}
