import { UtilitySidebar } from '@/shared/components/UtilitySidebar';
import { useWorkspaceWiki } from '@/features/wiki/ui/WorkspaceWikiProvider';
import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { FileTreeContainer } from './FileTreeContainer';
import { ProcessListContainer } from './ProcessListContainer';
import { GitPanelContainer } from './GitPanelContainer';
import { TerminalSection } from '@/shared/components/TerminalSection';
import { WorkspaceNotesContainer } from './WorkspaceNotesContainer';
import { WorkspaceWikiNavigation } from '@/features/wiki/ui/WorkspaceWikiPanel';
import { useDiffs } from '@/shared/stores/useWorkspaceDiffStore';
import { ArrowsOutSimpleIcon } from '@phosphor-icons/react';
import { useLogsPanel } from '@/shared/hooks/useLogsPanel';
import {
  useWorkspaceFilesSelection,
  WorkspaceFileBrowserPanel,
} from '@/features/workspace-files';
import type { RepoWithTargetBranch, Workspace } from 'shared/types';
import {
  PERSIST_KEYS,
  PersistKey,
  RIGHT_MAIN_PANEL_MODES,
  type RightMainPanelMode,
  usePersistedExpanded,
  useUiPreferencesStore,
} from '@/shared/stores/useUiPreferencesStore';
import {
  CollapsibleSectionHeader,
  type SectionAction,
} from '@vibe/ui/components/CollapsibleSectionHeader';

type SectionDef = {
  title: string;
  persistKey: PersistKey;
  visible: boolean;
  expanded: boolean;
  content: React.ReactNode;
  actions: SectionAction[];
};

export interface RightSidebarProps {
  rightMainPanelMode: RightMainPanelMode | null;
  selectedWorkspace: Workspace | undefined;
  repos: RepoWithTargetBranch[];
  onWikiPageSelected?: () => void;
}

export const RightSidebar = memo(function RightSidebar({
  rightMainPanelMode,
  selectedWorkspace,
  repos,
  onWikiPageSelected,
}: RightSidebarProps) {
  const { t } = useTranslation(['tasks', 'common']);
  const diffs = useDiffs();
  const wiki = useWorkspaceWiki();
  const isTerminalVisible = useUiPreferencesStore((s) => s.isTerminalVisible);
  const setRightMainPanelMode = useUiPreferencesStore(
    (s) => s.setRightMainPanelMode
  );
  const { expandTerminal, isTerminalExpanded } = useLogsPanel();
  const { target: selectedFileTarget, openTarget: openFileTarget } =
    useWorkspaceFilesSelection(selectedWorkspace?.id);

  const [changesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.changesSection,
    true
  );
  const [filesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.rightPanelFiles,
    true
  );
  const [processesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.processesSection,
    true
  );
  const [gitExpanded] = usePersistedExpanded(
    PERSIST_KEYS.gitPanelRepositories,
    true
  );
  const [notesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.notesSection,
    false
  );

  const hasUpperContent =
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES ||
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.FILES ||
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS;

  const upperExpanded = (() => {
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES)
      return changesExpanded;
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.FILES)
      return filesExpanded;
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS)
      return processesExpanded;
    return false;
  })();

  const sections: SectionDef[] = useMemo(() => {
    const result: SectionDef[] = [
      {
        title: 'Git',
        persistKey: PERSIST_KEYS.gitPanelRepositories,
        visible: true,
        expanded: gitExpanded,
        content: (
          <GitPanelContainer
            selectedWorkspace={selectedWorkspace}
            repos={repos}
          />
        ),
        actions: [],
      },
      {
        title: t('common:sections.notes'),
        persistKey: PERSIST_KEYS.notesSection,
        visible: true,
        expanded: notesExpanded,
        content: <WorkspaceNotesContainer />,
        actions: [],
      },
    ];

    switch (rightMainPanelMode) {
      case RIGHT_MAIN_PANEL_MODES.CHANGES:
        if (selectedWorkspace) {
          result.unshift({
            title: t('common:sections.changes'),
            persistKey: PERSIST_KEYS.changesSection,
            visible: hasUpperContent,
            expanded: upperExpanded,
            content: (
              <FileTreeContainer
                key={selectedWorkspace.id}
                workspaceId={selectedWorkspace.id}
                diffs={diffs}
                className=""
              />
            ),
            actions: [],
          });
        }
        break;
      case RIGHT_MAIN_PANEL_MODES.FILES:
        break;
      case RIGHT_MAIN_PANEL_MODES.LOGS:
        result.unshift({
          title: t('common:sections.logs'),
          persistKey: PERSIST_KEYS.rightPanelprocesses,
          visible: hasUpperContent,
          expanded: upperExpanded,
          content: <ProcessListContainer />,
          actions: [],
        });
        break;
      case RIGHT_MAIN_PANEL_MODES.WIKI:
      case RIGHT_MAIN_PANEL_MODES.TODOS:
      case null:
        break;
    }

    if (selectedWorkspace) {
      result.unshift({
        title: t('common:fileTree.title'),
        persistKey: PERSIST_KEYS.rightPanelFiles,
        visible: true,
        expanded: filesExpanded,
        content: (
          <WorkspaceFileBrowserPanel
            key={selectedWorkspace.id}
            workspaceId={selectedWorkspace.id}
            selectedTarget={selectedFileTarget}
            source="file-tree"
            className="border-r-0"
            onSelectFile={(target) => {
              openFileTarget(target);
              setRightMainPanelMode(
                RIGHT_MAIN_PANEL_MODES.FILES,
                selectedWorkspace.id
              );
            }}
          />
        ),
        actions: [],
      });
    }

    return result;
  }, [
    rightMainPanelMode,
    selectedWorkspace,
    repos,
    diffs,
    gitExpanded,
    filesExpanded,
    notesExpanded,
    hasUpperContent,
    upperExpanded,
    selectedFileTarget,
    openFileTarget,
    setRightMainPanelMode,
    t,
  ]);

  return (
    <UtilitySidebar>
      {wiki.repositoryOptions.length > 0 && (
        <CollapsibleSectionHeader
          className="!h-auto"
          title={t('common:environment.wiki')}
          persistKey="workspace-wiki"
          defaultExpanded={false}
          forceExpanded={rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.WIKI}
        >
          <div className="h-80 min-h-0">
            <WorkspaceWikiNavigation
              onSelectPage={() => {
                if (selectedWorkspace)
                  setRightMainPanelMode('wiki', selectedWorkspace.id);
                onWikiPageSelected?.();
              }}
            />
          </div>
        </CollapsibleSectionHeader>
      )}
      {selectedWorkspace?.usage === 'interactive' &&
        selectedWorkspace.container_ref &&
        isTerminalVisible &&
        !isTerminalExpanded && (
          <TerminalSection
            scope={selectedWorkspace.id}
            workspaceId={selectedWorkspace.id}
            cwd={selectedWorkspace.container_ref}
            actions={[{ icon: ArrowsOutSimpleIcon, onClick: expandTerminal }]}
          />
        )}
      <div className="divide-y border-b">
        {sections
          .filter((section) => section.visible)
          .map((section) => (
            <div
              key={section.persistKey}
              className="max-h-[max(50vh,400px)] flex flex-col overflow-hidden"
            >
              <CollapsibleSectionHeader
                title={section.title}
                persistKey={section.persistKey}
                defaultExpanded={section.expanded}
                actions={section.actions}
              >
                <div className="flex flex-1 border-t min-h-[200px] w-full overflow-auto">
                  {section.content}
                </div>
              </CollapsibleSectionHeader>
            </div>
          ))}
      </div>
    </UtilitySidebar>
  );
});
