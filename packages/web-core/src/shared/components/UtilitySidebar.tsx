import type { ReactNode } from 'react';
import { EnvironmentPanel } from './EnvironmentPanel';
export function UtilitySidebar({ children }: { children: ReactNode }) {
  return (
    <aside className="h-full min-h-0 border-l bg-secondary overflow-y-auto">
      <EnvironmentPanel />
      {children}
    </aside>
  );
}
