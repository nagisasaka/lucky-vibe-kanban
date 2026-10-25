'use client';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { ButtonGroup, ButtonGroupItem } from './IconButtonGroup';
export type ViewNavMode = 'kanban' | 'list';
export type IssueArchiveScope = 'active' | 'archived' | 'all';
export interface ViewNavTabsProps {
  scope: IssueArchiveScope;
  onScopeChange: (scope: IssueArchiveScope) => void;
  className?: string;
}
export function ViewNavTabs({
  scope,
  onScopeChange,
  className,
}: ViewNavTabsProps) {
  const { t } = useTranslation('common');
  return (
    <div className="scrollbar-none flex min-w-0 items-center gap-base overflow-x-auto overflow-y-hidden">
      <ButtonGroup className={cn('shrink-0', className)}>
        {(['active', 'archived', 'all'] as const).map((value) => (
          <ButtonGroupItem
            key={value}
            active={scope === value}
            onClick={() => onScopeChange(value)}
            className="shrink-0 whitespace-nowrap"
          >
            {t(`kanban.viewTabs.${value}`)}
          </ButtonGroupItem>
        ))}
      </ButtonGroup>
    </div>
  );
}
