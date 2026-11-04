import { WorkspaceFilesSelectionProvider } from '../../packages/web-core/src/features/workspace-files';
import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '../../packages/web-core/src/i18n/config';
import { PetSettings } from '../../packages/web-core/src/features/pets/SidebarPet';
import { ProjectUtilities } from '../../packages/web-core/src/pages/kanban/ProjectUtilities';
import { RightSidebar } from '../../packages/web-core/src/pages/workspaces/RightSidebar';
import { useUiPreferencesStore } from '../../packages/web-core/src/shared/stores/useUiPreferencesStore';
import { registerPetPreferencesWriter } from '../../packages/web-core/src/shared/lib/petPreferencesPersistence';
void i18n.changeLanguage('en');
for (const key of [
  'kanban-main-files',
  'git-panel-repositories',
  'notes-section',
  'right-panel-files',
  'changes-section',
  'processes-section',
])
  localStorage.setItem('vibe.ui.collapsible.' + key, 'false');
// All state is scoped to this ephemeral origin, while the product store and writer boundary are real.
useUiPreferencesStore.setState({
  petPreferences: JSON.parse(localStorage.getItem('integration-pet') || 'null'),
});
registerPetPreferencesWriter(async (preferences) => {
  localStorage.setItem('integration-pet', JSON.stringify(preferences));
});
const fixtureRepos = [
  {
    id: 'repo-one',
    name: 'First',
    path: '/isolated/first',
    wiki_enabled: false,
  },
  {
    id: 'repo-two',
    name: 'Second',
    path: '/isolated/second',
    wiki_enabled: false,
  },
];
function App() {
  const [repoCount, setRepoCount] = useState(0);
  const [screen, setScreen] = useState('Kanban');
  const open = useUiPreferencesStore((s) => s.isRightSidebarVisible);
  const toggle = useUiPreferencesStore((s) => s.toggleRightSidebar);
  return (
    <main className="bg-primary text-normal h-screen flex flex-col">
      <nav className="flex flex-wrap gap-base p-base">
        <button onClick={() => setScreen('Kanban')}>Kanban screen</button>
        <button onClick={() => setScreen('Workspace')}>Workspace screen</button>
        <button onClick={toggle}>Toggle sidebar</button>
        {[0, 1, 2].map((count) => (
          <button key={count} onClick={() => setRepoCount(count)}>
            Repositories {count}
          </button>
        ))}
      </nav>
      <details
        open
        id="pet-settings-fixture"
        className="fixed left-0 bottom-0 z-50 w-80 max-w-full bg-primary border max-h-[70vh] overflow-auto"
      >
        <summary>Host and pet fixture</summary>
        <PetSettings />
      </details>
      <div className="flex flex-1 min-h-0">
        <article className="hidden md:block flex-1 min-w-0 p-base">
          <h1>{screen}</h1>
          <p>Isolated screen shell; actual utility sidebar components.</p>
        </article>
        {open && (
          <div
            data-testid="right-sidebar-shell"
            className="w-[300px] max-w-[100vw] shrink-0 h-full overflow-hidden"
          >
            {screen === 'Kanban' ? (
              <ProjectUtilities
                projectId="fixture-project"
                repos={fixtureRepos.slice(0, repoCount)}
                error={false}
                selectedFile={null}
                onFile={(target) => {
                  document.title = 'Selected ' + target.repoId;
                }}
                onWiki={() => {}}
                onClose={toggle}
              />
            ) : (
              <RightSidebar
                rightMainPanelMode={null}
                selectedWorkspace={undefined}
                repos={[]}
              />
            )}
          </div>
        )}
      </div>
    </main>
  );
}
createRoot(document.getElementById('root')!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <WorkspaceFilesSelectionProvider>
      <App />
    </WorkspaceFilesSelectionProvider>
  </QueryClientProvider>
);
