import { useUiPreferencesStore } from '@/shared/stores/useUiPreferencesStore';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { TerminalSection } from '@/shared/components/TerminalSection';
import { UtilitySidebar } from '@/shared/components/UtilitySidebar';
import { useTerminal } from '@/shared/hooks/useTerminal';
import {
  WorkspaceFileBrowserPanel,
  type WorkspaceFilePreviewTarget,
} from '@/features/workspace-files';
import { WorkspaceWikiNavigation } from '@/features/wiki/ui/WorkspaceWikiPanel';
import type { MainRepo } from '@/shared/lib/projectUtilitiesApi';

export function ProjectUtilities({
  projectId,
  repos,
  error,
  selectedFile,
  onFile,
  onWiki,
}: {
  projectId: string;
  repos: MainRepo[];
  error: boolean;
  selectedFile: WorkspaceFilePreviewTarget | null;
  onFile: (target: WorkspaceFilePreviewTarget) => void;
  onWiki: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation('common');
  const [repoId, setRepoId] = useState('');
  const wikiKey = `kanban-wiki:${projectId}`;
  const wikiExpanded = useUiPreferencesStore(
    (s) => s.expanded[wikiKey] ?? false
  );
  const setExpanded = useUiPreferencesStore((s) => s.setExpanded);
  const terminal = useTerminal();
  const terminalOpen =
    terminal.getTabsForWorkspace(`main:${projectId}`).length > 0;
  const repo = repos.find((r) => r.id === repoId) ?? repos[0];
  return (
    <UtilitySidebar>
      {error && (
        <p role="status" className="p-base text-error">
          {t('environment.unavailable')}
        </p>
      )}
      <CollapsibleSectionHeader
        className="!h-auto"
        title={t('environment.files')}
        persistKey="kanban-main-files"
      >
        <div className="h-80">
          <WorkspaceFileBrowserPanel
            workspaceId={projectId}
            source="file-tree"
            selectedTarget={selectedFile}
            onSelectFile={onFile}
          />
        </div>
      </CollapsibleSectionHeader>
      {repo && (
        <>
          <div className="p-base text-sm text-low">
            <select
              aria-label={t('environment.mainTerminal')}
              data-testid="main-terminal-repository"
              disabled={terminalOpen}
              className="w-full bg-primary text-normal border rounded"
              value={repo.id}
              onChange={(e) => setRepoId(e.target.value)}
            >
              {repos.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}
                </option>
              ))}
            </select>
          </div>
          <TerminalSection
            scope={`main:${projectId}`}
            cwd={repo.path}
            projectId={projectId}
            repoId={repo.id}
          />
        </>
      )}
      {repos.some((r) => r.wiki_enabled === true) && (
        <CollapsibleSectionHeader
          className="!h-auto"
          title={t('environment.wiki')}
          persistKey={wikiKey}
          expanded={wikiExpanded}
          onExpandedChange={(value) => setExpanded(wikiKey, value)}
        >
          <div className="h-80">
            <WorkspaceWikiNavigation onSelectPage={onWiki} />
          </div>
        </CollapsibleSectionHeader>
      )}
    </UtilitySidebar>
  );
}
