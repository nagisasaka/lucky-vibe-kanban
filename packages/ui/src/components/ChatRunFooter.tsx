import { useTranslation } from 'react-i18next';
import { ChatElapsedTime } from './ChatElapsedTime';

export function ChatRunFooter({
  startedAt,
  endedAt,
  active,
}: {
  startedAt: string;
  endedAt: string | null;
  active: boolean;
}) {
  const { t } = useTranslation('tasks');
  const completed =
    endedAt && Number.isFinite(Date.parse(endedAt)) ? endedAt : null;
  return (
    <div className="flex flex-wrap items-center gap-base text-xs text-low">
      <ChatElapsedTime
        startedAt={startedAt}
        endedAt={endedAt}
        active={active}
      />
      {completed && (
        <time dateTime={completed}>
          {t('execution.completedAt', {
            time: new Date(completed).toLocaleString(),
          })}
        </time>
      )}
    </div>
  );
}
