import type { ReactNode } from 'react';
import { EnvironmentPanel } from './EnvironmentPanel';
export function UtilitySidebar({
  children,
  repoIds = [],
}: {
  children: ReactNode;
  repoIds?: string[];
}) {
  return (
    <aside className="flex h-full min-h-0 flex-col border-l bg-secondary overflow-y-auto [&>*]:shrink-0">
      <EnvironmentPanel repoIds={repoIds} />
      {children}
    </aside>
  );
}
