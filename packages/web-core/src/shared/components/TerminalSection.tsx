import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CollapsibleSectionHeader,
  type SectionAction,
} from '@vibe/ui/components/CollapsibleSectionHeader';
import { XTermInstance } from './XTermInstance';
import { useTerminal } from '@/shared/hooks/useTerminal';

/** Only a header click starts/stops a shell. Mounting this surface never does. */
export function TerminalSection({
  scope,
  cwd,
  workspaceId,
  projectId,
  repoId,
  children,
  actions = [],
}: {
  scope: string;
  cwd: string;
  workspaceId?: string;
  projectId?: string;
  repoId?: string;
  children?: ReactNode;
  actions?: SectionAction[];
}) {
  const { t } = useTranslation('common');
  const terminal = useTerminal();
  const tab = terminal.getTabsForWorkspace(scope)[0];
  return (
    <CollapsibleSectionHeader
      className="!h-auto"
      title={t('sections.terminal')}
      expanded={Boolean(tab)}
      onExpandedChange={(open) => {
        if (open) terminal.createTab(scope, cwd);
        else if (tab) terminal.closeTab(scope, tab.id);
      }}
      actions={tab ? actions : []}
    >
      {children}
      {tab && (
        <div className="h-80">
          <XTermInstance
            tabId={tab.id}
            workspaceId={workspaceId ?? scope}
            projectId={projectId}
            repoId={repoId}
            isActive
            onClose={() => terminal.closeTab(scope, tab.id)}
          />
        </div>
      )}
    </CollapsibleSectionHeader>
  );
}
