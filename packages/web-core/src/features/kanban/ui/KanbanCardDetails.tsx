import { useTranslation } from 'react-i18next';

export function KanbanCardDetails({
  count,
  description,
}: {
  count: number;
  description?: string | null;
}) {
  const { t } = useTranslation('common');
  return (
    <details onClick={(event) => event.stopPropagation()}>
      <summary className="cursor-pointer text-sm text-low rounded-sm focus-visible:ring-2 focus-visible:ring-brand">
        {t('kanban.compact.workspaces', { count })}
      </summary>
      {description && (
        <p className="text-sm text-normal whitespace-pre-wrap break-words mt-half">
          {description}
        </p>
      )}
    </details>
  );
}
