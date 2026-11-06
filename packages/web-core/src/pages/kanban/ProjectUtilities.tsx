import { useUiPreferencesStore } from '@/shared/stores/useUiPreferencesStore';
import { CaretDownIcon, FolderSimpleIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { TerminalSection } from '@/shared/components/TerminalSection';
import { UtilitySidebar } from '@/shared/components/UtilitySidebar';
import { mainTerminalRepo } from '@/shared/lib/mainTerminalRepo';
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
  const wikiKey = `kanban-wiki:${projectId}`;
  const wikiExpanded = useUiPreferencesStore(
    (s) => s.expanded[wikiKey] ?? false
  );
  const setExpanded = useUiPreferencesStore((s) => s.setExpanded);
  const repo = mainTerminalRepo(repos, error);
  return (
    <UtilitySidebar repoIds={repos.map((repo) => repo.id)}>
      {error && (
        <p role="status" className="p-base text-error">
          {t('environment.unavailable')}
        </p>
      )}
      {repos.length > 0 && (
        <details
          className="group border-b px-base py-base text-xs text-normal"
          data-testid="main-repository-details"
        >
          <summary className="flex cursor-pointer list-none items-center gap-half rounded-sm focus-visible:ring-2 focus-visible:ring-brand [&::-webkit-details-marker]:hidden">
            <FolderSimpleIcon className="size-icon-xs shrink-0 text-low" />
            <span className="flex-1 font-medium">
              {t('utilityPanel.mainRepositories', { count: repos.length })}
            </span>
            <CaretDownIcon className="size-icon-2xs text-low group-open:rotate-180" />
          </summary>
          <dl className="mt-base space-y-base border-l pl-base">
            {repos.map((repository) => (
              <div key={repository.id} className="space-y-half">
                <dt className="font-medium text-high">{repository.name}</dt>
                <dd className="break-all font-ibm-plex-mono text-low">
                  {repository.path}
                </dd>
              </div>
            ))}
          </dl>
        </details>
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
        <TerminalSection
          scope={`main:${projectId}`}
          cwd={repo.path}
          projectId={projectId}
          repoId={repo.id}
        />
      )}
      {!repo && (
        <button
          type="button"
          disabled
          aria-expanded={false}
          className="flex w-full items-center justify-between px-base py-half text-low disabled:cursor-not-allowed"
        >
          <span className="font-medium">{t('sections.terminal')}</span>
        </button>
      )}
      {repos.some((r) => r.wiki_enabled === true) && (
        <CollapsibleSectionHeader
          className="!h-auto !min-h-fit has-[>div>button[aria-expanded=true]]:flex-1"
          title={t('environment.wiki')}
          persistKey={wikiKey}
          expanded={wikiExpanded}
          onExpandedChange={(value) => setExpanded(wikiKey, value)}
        >
          {/* Keep tree content out of intrinsic sizing while preserving controls and a scrollable row. */}
          <div className="flex flex-1 flex-col [&_nav]:[contain:size] [&_nav]:min-h-[1lh]">
            <WorkspaceWikiNavigation onSelectPage={onWiki} />
          </div>
        </CollapsibleSectionHeader>
      )}
    </UtilitySidebar>
  );
}
