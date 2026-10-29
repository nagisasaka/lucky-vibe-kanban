import { useEffect } from 'react';
import { useMobileActiveTab } from '@/shared/stores/useUiPreferencesStore';
import { Workspaces } from './Workspaces';

export function WorkspacesLanding() {
  const [, setMobileActiveTab] = useMobileActiveTab();

  // The list route has no selected workspace to show in the chat/tools tabs.
  // Reset on entry so returning from a detail pane also reveals the list.
  useEffect(() => {
    setMobileActiveTab('workspaces');
  }, [setMobileActiveTab]);

  return <Workspaces />;
}
