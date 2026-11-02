import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useTerminal } from '@/shared/hooks/useTerminal';
import { TerminalPanel } from '@vibe/ui/components/TerminalPanel';
import { XTermInstance } from './XTermInstance';

export function TerminalPanelContainer() {
  const { t } = useTranslation('common');
  const { workspace } = useWorkspaceContext();
  const { getTabsForWorkspace, closeTab } = useTerminal();
  const workspaceId = workspace?.id;
  const interactive = workspace?.usage === 'interactive';
  const tabs = workspaceId ? getTabsForWorkspace(workspaceId) : [];
  if (!interactive)
    return (
      <p className="p-base text-low">{t('workspaces.terminalUnavailable')}</p>
    );

  return (
    <TerminalPanel
      tabs={tabs}
      activeTabId={tabs[0]?.id ?? null}
      renderTab={(tabId, isActive) => (
        <XTermInstance
          key={tabId}
          tabId={tabId}
          workspaceId={workspaceId ?? ''}
          isActive={isActive}
          onClose={() => workspaceId && closeTab(workspaceId, tabId)}
        />
      )}
    />
  );
}
