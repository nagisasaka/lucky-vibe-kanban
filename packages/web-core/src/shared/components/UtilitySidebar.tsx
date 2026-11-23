import type { ReactNode } from 'react';
import './UtilitySidebar.css';
import { EnvironmentPanel } from './EnvironmentPanel';
export function UtilitySidebar({
  children,
  repoIds = [],
  hasPendingApproval = false,
}: {
  children: ReactNode;
  repoIds?: string[];
  hasPendingApproval?: boolean;
}) {
  return (
    <aside className="utility-sidebar flex h-full min-h-0 flex-col border-l bg-secondary overflow-y-auto [&>*]:shrink-0">
      <EnvironmentPanel
        repoIds={repoIds}
        hasPendingApproval={hasPendingApproval}
      />
      {children}
    </aside>
  );
}
