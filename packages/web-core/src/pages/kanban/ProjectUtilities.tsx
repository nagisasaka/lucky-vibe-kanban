import { useUiPreferencesStore } from '@/shared/stores/useUiPreferencesStore';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { TerminalPanel } from '@vibe/ui/components/TerminalPanel';
import { UtilitySidebar } from '@/shared/components/UtilitySidebar';
import { XTermInstance } from '@/shared/components/XTermInstance';
import { useTerminal } from '@/shared/hooks/useTerminal';
import {
  WorkspaceFileBrowserPanel,
  type WorkspaceFilePreviewTarget,
} from '@/features/workspace-files';
import { WorkspaceWikiNavigation } from '@/features/wiki/ui/WorkspaceWikiPanel';
import type { MainRepo } from '@/shared/lib/projectUtilitiesApi';

function MainTerminal({
  projectId,
  repo,
}: {
  projectId: string;
  repo: MainRepo;
}) {
  const terminal = useTerminal();
  const { createTab, clearWorkspaceTabs } = terminal;
  const clearTabs = useRef(clearWorkspaceTabs);
  useEffect(() => {
    clearTabs.current = clearWorkspaceTabs;
  }, [clearWorkspaceTabs]);
  const scope = `main:${projectId}:${repo.id}`;
  const tabs = terminal.getTabsForWorkspace(scope);
  const active = terminal.getActiveTab(scope);
  useEffect(() => {
    createTab(scope, repo.path);
    return () => clearTabs.current(scope);
  }, [scope, repo.path, createTab]);
  return (
    <div className="h-80">
      <TerminalPanel
        tabs={tabs}
        activeTabId={active?.id ?? null}
        renderTab={(id, isActive) => (
          <XTermInstance
            key={id}
            tabId={id}
            workspaceId={scope}
            projectId={projectId}
            repoId={repo.id}
            isActive={isActive}
            onClose={() => terminal.closeTab(scope, id)}
          />
        )}
      />
    </div>
  );
}

export function ProjectUtilities({
  projectId,
  repos,
  error,
  selectedFile,
  onFile,
  onWiki,
  onClose,
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
  const repo = repos.find((r) => r.id === repoId) ?? repos[0];
  return (
    <UtilitySidebar>
      <button
        type="button"
        onClick={onClose}
        className="w-full p-base border-b text-normal hover:bg-panel"
      >
        {t('environment.close')}
      </button>
      <div className="p-base text-sm text-low space-y-half">
        {repos.map((repo) => (
          <p key={repo.id} className="break-all">
            {repo.name} · {t('environment.mainScope', { path: repo.path })}
          </p>
        ))}
      </div>
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
      <CollapsibleSectionHeader
        className="!h-auto"
        title={t('sections.terminal')}
        persistKey="kanban-main-terminal"
        defaultExpanded={false}
      >
        <div className="p-base text-sm text-low">
          <p>{t('environment.mainTerminal')}</p>
          <select
            aria-label={t('workspaceWiki.repository')}
            className="w-full bg-primary text-normal border rounded"
            value={repo?.id ?? ''}
            onChange={(e) => setRepoId(e.target.value)}
          >
            {repos.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
          {repo && (
            <p className="break-all">
              {t('environment.mainScope', { path: repo.path })}
            </p>
          )}
        </div>
        {repo && (
          <MainTerminal key={repo.id} projectId={projectId} repo={repo} />
        )}
      </CollapsibleSectionHeader>
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
