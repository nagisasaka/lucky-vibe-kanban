import { useEffect, useMemo, useRef, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import type { GitBranch, Repo } from 'shared/types';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import BranchSelector from '@/shared/components/tasks/BranchSelector';
import { ServerFolderPickerDialog } from './ServerFolderPickerDialog';
import { fileSystemApi, repoApi } from '@/shared/lib/api';
import { getProjectWorkspaceDefaultOrThrow } from '@/shared/hooks/useProjectRepoDefaults';
import { defineModal } from '@/shared/lib/modals';

export type WorkspaceTargetMode = 'worktree' | 'direct_folder';
export type WorkspaceTargetDialogPurpose = 'execution' | 'project_default';
type GitSelection = { repo: Repo; targetBranch: string };
export type WorkspaceTargetSelection =
  | ({ mode: 'worktree'; path: string; repos: GitSelection[] } & GitSelection)
  | { mode: 'direct_folder'; path: string };
export type WorkspaceTargetDialogResult =
  | { kind: 'confirmed'; selection: WorkspaceTargetSelection }
  | { kind: 'canceled' };
export interface WorkspaceTargetDialogProps {
  initialPath?: string;
  initialMode?: WorkspaceTargetMode;
  initialBranch?: string | null;
  initialRepos?: Array<{ repo_id: string; target_branch: string | null }>;
  projectId?: string | null;
  hostId?: string | null;
  allowedModes?: WorkspaceTargetMode[];
  purpose?: WorkspaceTargetDialogPurpose;
  singleRepository?: boolean;
  title?: string;
  description?: string;
}

function RepositoryChoice({
  repo,
  selected,
  branch,
  onToggle,
  onBranch,
  branchesRequired,
  hostId,
}: {
  repo: Repo;
  selected: boolean;
  branch: string;
  onToggle: () => void;
  onBranch: (value: string) => void;
  branchesRequired: boolean;
  hostId?: string | null;
}) {
  const { t } = useTranslation('common');
  const [branches, setBranches] = useState<GitBranch[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!selected || !branchesRequired) return;
    let canceled = false;
    setError(null);
    void repoApi
      .getBranches(repo.id, hostId)
      .then((items) => {
        if (canceled) return;
        setBranches(items);
      })
      .catch((e: unknown) => {
        if (!canceled) setError(String(e));
      });
    return () => {
      canceled = true;
    };
  }, [selected, branchesRequired, repo.id, hostId]);
  useEffect(() => {
    if (!selected || !branchesRequired || branches.length === 0) return;
    if (branches.some((item) => item.name === branch)) return;
    const next =
      branches.find((item) => item.name === repo.default_target_branch) ??
      branches.find((item) => item.is_current) ??
      branches[0];
    onBranch(next.name);
  }, [
    selected,
    branchesRequired,
    branches,
    branch,
    repo.default_target_branch,
    onBranch,
  ]);
  return (
    <div className="rounded-sm border border-border p-base">
      <label className="flex items-center gap-base text-sm text-high">
        <input type="checkbox" checked={selected} onChange={onToggle} />
        <span className="min-w-0 truncate" title={repo.path}>
          {repo.display_name || repo.name}
        </span>
      </label>
      {selected && branchesRequired && (
        <div className="mt-base">
          <span className="text-xs text-low">
            {t('createMode.workspaceDialog.branchLabel')}
          </span>
          <BranchSelector
            branches={branches}
            selectedBranch={branch || null}
            onBranchSelect={onBranch}
          />
          {error && (
            <p role="alert" className="text-xs text-error">
              {error}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

const WorkspaceTargetDialogImpl = create<WorkspaceTargetDialogProps>(
  ({
    initialPath = '',
    initialMode = 'worktree',
    initialBranch,
    initialRepos,
    projectId,
    hostId,
    allowedModes,
    purpose = 'execution',
    singleRepository = false,
    title,
    description,
  }) => {
    const modal = useModal();
    const { t } = useTranslation('common');
    const projectDefault = purpose === 'project_default';
    const [mode, setMode] = useState<WorkspaceTargetMode>(initialMode);
    const [path, setPath] = useState(initialPath);
    const [available, setAvailable] = useState<Repo[]>([]);
    const [selected, setSelected] = useState<Record<string, string>>({});
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const generation = useRef(0);
    const restricted = Boolean(projectId && !projectDefault);
    const canBrowse = !restricted;
    const directAllowed =
      !projectDefault &&
      (!allowedModes || allowedModes.includes('direct_folder'));

    useEffect(() => {
      if (!modal.visible) return;
      const id = ++generation.current;
      setMode(projectDefault ? 'worktree' : initialMode);
      setPath(initialPath);
      setError(null);
      setBusy(true);
      void Promise.all([
        repoApi.list(hostId),
        projectId
          ? getProjectWorkspaceDefaultOrThrow(projectId, hostId)
          : Promise.resolve(null),
      ])
        .then(([all, preset]) => {
          if (generation.current !== id) return;
          const presetRepos =
            preset?.kind === 'git' ? (preset.repos ?? [preset.repo]) : [];
          const allowed = restricted
            ? new Set(presetRepos.map((r) => r.repo_id))
            : null;
          const repos = allowed ? all.filter((r) => allowed.has(r.id)) : all;
          setAvailable(repos);
          const seeds =
            initialRepos ??
            (restricted
              ? presetRepos
              : repos
                  .filter((r) => r.path === initialPath)
                  .map((r) => ({
                    repo_id: r.id,
                    target_branch: initialBranch ?? '',
                  })));
          const validSeeds = seeds.filter((s) =>
            repos.some((r) => r.id === s.repo_id)
          );
          setSelected(
            Object.fromEntries(
              (singleRepository ? validSeeds.slice(0, 1) : validSeeds).map(
                (s) => [
                  s.repo_id,
                  projectDefault ? '' : (s.target_branch ?? ''),
                ]
              )
            )
          );
        })
        .catch((e: unknown) => {
          if (generation.current === id) setError(String(e));
        })
        .finally(() => {
          if (generation.current === id) setBusy(false);
        });
      return () => {
        generation.current += 1;
      };
    }, [
      modal.visible,
      projectId,
      hostId,
      restricted,
      projectDefault,
      initialMode,
      initialPath,
      initialBranch,
      initialRepos,
      singleRepository,
    ]);

    const choices = useMemo(
      () =>
        available
          .filter((r) => r.id in selected)
          .map((repo) => ({ repo, targetBranch: selected[repo.id] })),
      [available, selected]
    );
    const cancel = () => {
      generation.current += 1;
      modal.resolve({ kind: 'canceled' } satisfies WorkspaceTargetDialogResult);
      modal.hide();
    };
    const browse = async () => {
      const id = generation.current;
      try {
        const chosen = await ServerFolderPickerDialog.show({
          value: path,
          hostId,
          title: t('createMode.workspaceDialog.nativePickerTitle'),
        });
        if (!chosen || id !== generation.current) return;
        setPath(chosen);
        if (mode === 'direct_folder') return;
        setBusy(true);
        setError(null);
        const inspected = await fileSystemApi.inspectDirectory(
          { path: chosen },
          hostId
        );
        if (id !== generation.current) return;
        if (!inspected.repo)
          throw new Error(t('createMode.workspaceDialog.worktreeUnavailable'));
        const repo = inspected.repo;
        setAvailable((all) =>
          all.some((r) => r.id === repo.id) ? all : [...all, repo]
        );
        setSelected((previous) =>
          singleRepository ? { [repo.id]: '' } : { ...previous, [repo.id]: '' }
        );
      } catch (e) {
        if (id === generation.current) setError(String(e));
      } finally {
        if (id === generation.current) setBusy(false);
      }
    };
    const confirm = () => {
      if (mode === 'direct_folder')
        modal.resolve({
          kind: 'confirmed',
          selection: { mode, path: path.trim() },
        } satisfies WorkspaceTargetDialogResult);
      else {
        const first = choices[0];
        if (!first || (singleRepository && choices.length !== 1)) return;
        modal.resolve({
          kind: 'confirmed',
          selection: { mode, path: first.repo.path, ...first, repos: choices },
        } satisfies WorkspaceTargetDialogResult);
      }
      modal.hide();
    };
    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => {
          if (!open) cancel();
        }}
      >
        <DialogContent
          role="dialog"
          aria-modal="true"
          className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-xl"
        >
          <DialogHeader>
            <DialogTitle>
              {title ?? t('createMode.workspaceDialog.title')}
            </DialogTitle>
            <DialogDescription>
              {description ?? t('createMode.workspaceDialog.description')}
            </DialogDescription>
          </DialogHeader>
          {!projectDefault && (
            <div
              role="tablist"
              className="flex gap-half border-b border-border"
            >
              {(
                [
                  'worktree',
                  ...(directAllowed ? (['direct_folder'] as const) : []),
                ] as WorkspaceTargetMode[]
              ).map((value) => (
                <Button
                  key={value}
                  role="tab"
                  aria-selected={mode === value}
                  variant={mode === value ? 'default' : 'outline'}
                  disabled={busy}
                  onClick={() => {
                    setMode(value);
                    setError(null);
                  }}
                >
                  {value === 'worktree'
                    ? t('createMode.workspaceDialog.gitTab')
                    : t('createMode.workspaceDialog.folderTab')}
                </Button>
              ))}
            </div>
          )}
          {mode === 'worktree' ? (
            <div className="flex flex-col gap-base">
              <p className="text-xs text-low">
                {t(
                  projectDefault
                    ? 'createMode.workspaceDialog.presetHint'
                    : 'createMode.workspaceDialog.worktreeDescription'
                )}
              </p>
              {available.map((repo) => (
                <RepositoryChoice
                  key={repo.id}
                  repo={repo}
                  selected={repo.id in selected}
                  branch={selected[repo.id] ?? ''}
                  branchesRequired={!projectDefault}
                  hostId={hostId}
                  onToggle={() =>
                    setSelected((previous) => {
                      const next = { ...previous };
                      if (repo.id in next) delete next[repo.id];
                      else {
                        if (singleRepository) return { [repo.id]: '' };
                        next[repo.id] = '';
                      }
                      return next;
                    })
                  }
                  onBranch={(value) =>
                    setSelected((previous) =>
                      repo.id in previous
                        ? { ...previous, [repo.id]: value }
                        : previous
                    )
                  }
                />
              ))}
              {!busy && available.length === 0 && (
                <p className="text-sm text-low">
                  {t('createMode.workspaceDialog.emptyRepos')}
                </p>
              )}
              {canBrowse && (
                <Button
                  variant="outline"
                  onClick={() => void browse()}
                  disabled={busy}
                >
                  {t('serverFolderPicker.browse')}
                </Button>
              )}
            </div>
          ) : (
            <div className="flex flex-col gap-base">
              <p className="text-sm text-normal">
                {t('createMode.workspaceDialog.directDescription')}
              </p>
              <Input
                value={path}
                onChange={(e) => setPath(e.target.value)}
                aria-label={t('createMode.workspaceDialog.pathLabel')}
                placeholder={t('createMode.workspaceDialog.pathPlaceholder')}
              />
              <Button variant="outline" onClick={() => void browse()}>
                {t('serverFolderPicker.browse')}
              </Button>
            </div>
          )}
          {error && (
            <p role="alert" className="text-sm text-error">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={cancel}>
              {t('buttons.cancel')}
            </Button>
            <Button
              onClick={confirm}
              disabled={
                busy ||
                (mode === 'direct_folder'
                  ? !path.trim()
                  : choices.length === 0 ||
                    (singleRepository && choices.length !== 1) ||
                    (!projectDefault && choices.some((r) => !r.targetBranch)))
              }
            >
              {projectDefault
                ? t('createMode.workspaceDialog.useProjectLocation')
                : t('createMode.workspaceDialog.useWorkspace')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);
export const WorkspaceTargetDialog = defineModal<
  WorkspaceTargetDialogProps,
  WorkspaceTargetDialogResult
>(WorkspaceTargetDialogImpl);
