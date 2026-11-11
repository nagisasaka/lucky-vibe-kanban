import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { useTranslation } from 'react-i18next';
import {
  useEffect,
  useRef,
  useState,
  useCallback,
  type ComponentProps,
} from 'react';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { PROJECT_ISSUES_SHAPE } from 'shared/remote-types';
import { refreshShapeFallback } from '@/shared/lib/electric/collections';
import { makeRequest } from '@/shared/lib/remoteApi';
import { acknowledgeVersionSave } from './issue-version-drafts';

interface Version {
  id: string;
  issue_id: string;
  name: string;
  description: string | null;
  revision: number;
}
interface VersionState {
  created_version_id?: string;
  versions: Version[];
  current_version_id: string;
  current_revision: number;
}
interface Draft {
  name: string;
  description: string;
  revision: number;
  saved: boolean;
}

export function IssueVersionEditor({
  issueId,
  projectId,
  editorProps,
  onCurrentDescription,
  observedIssueRevision,
}: {
  issueId: string;
  projectId: string;
  observedIssueRevision: string | undefined;
  onCurrentDescription: (description: string | null) => void;
  editorProps: ComponentProps<typeof WYSIWYGEditor>;
}) {
  const { t } = useTranslation('common');
  const [state, setState] = useState<VersionState | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [busyVersion, setBusyVersion] = useState<string | null>(null);
  const lock = useRef(false);
  const requestSequence = useRef(0);
  const [error, setError] = useState<{
    versionId: string | null;
    message: string;
  } | null>(null);
  const base = `/v1/issues/${issueId}/versions`;
  const request = useCallback(
    async function request(
      path = '',
      body?: unknown,
      method = 'GET'
    ): Promise<VersionState> {
      const response = await makeRequest(base + path, {
        method,
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
      if (!response.ok) {
        const detail = await response.json().catch(() => null);
        throw new Error(
          detail?.message ||
            t('issueVersions.saveFailed', { status: response.status })
        );
      }
      return response.json();
    },
    [base, t]
  );
  const accept = useCallback(
    (next: VersionState) => {
      requestSequence.current += 1;
      setState(next);
      onCurrentDescription(
        next.versions.find((version) => version.id === next.current_version_id)
          ?.description ?? null
      );
      setDrafts((previous) => {
        const merged = { ...previous };
        for (const version of next.versions) {
          if (!merged[version.id] || merged[version.id].saved) {
            merged[version.id] = {
              name: version.name,
              description: version.description ?? '',
              revision: version.revision,
              saved: true,
            };
          }
        }
        return merged;
      });
    },
    [onCurrentDescription]
  );
  useEffect(() => {
    if (lock.current) return;
    const sequence = ++requestSequence.current;
    let active = true;
    request()
      .then((next) => {
        if (!active || sequence !== requestSequence.current) return;
        accept(next);
        setSelected((previous) => previous ?? next.current_version_id);
      })
      .catch((cause) => {
        if (active && sequence === requestSequence.current)
          setError({ versionId: null, message: String(cause) });
      });
    return () => {
      active = false;
    };
    // Sync notifications refresh persisted versions without replacing dirty drafts.
  }, [issueId, observedIssueRevision, accept, request]);
  async function mutate(action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true;
    requestSequence.current += 1;
    setBusy(true);
    setBusyVersion(selected);
    setError(null);
    try {
      await action();
      await refreshShapeFallback(PROJECT_ISSUES_SHAPE, {
        project_id: projectId,
      });
    } catch (cause) {
      setError({ versionId: selected, message: String(cause) });
    } finally {
      // A sync notification may arrive while the mutation is locked. Refresh
      // after it settles so newer external changes are not lost.
      try {
        accept(await request());
      } catch (cause) {
        setError(
          (previous) =>
            previous ?? { versionId: selected, message: String(cause) }
        );
      }
      lock.current = false;
      setBusy(false);
      setBusyVersion(null);
    }
  }
  const draft = selected ? drafts[selected] : undefined;
  const version = state?.versions.find((item) => item.id === selected);
  function edit(patch: Partial<Draft>) {
    if (!selected) return;
    setDrafts((previous) => ({
      ...previous,
      [selected]: { ...previous[selected], ...patch, saved: false },
    }));
  }
  if (!state || !selected || !draft || !version)
    return (
      <div className="p-base text-low" role="status">
        {error?.message || t('issueVersions.loading')}
      </div>
    );
  return (
    <div className="space-y-base">
      <div
        className="flex flex-wrap gap-half px-base"
        role="tablist"
        aria-label={t('issueVersions.tabs')}
      >
        {state.versions.map((item) => (
          <PrimaryButton
            variant="tertiary"
            key={item.id}
            type="button"
            role="tab"
            aria-selected={selected === item.id}
            className={`rounded-sm border px-base py-half focus-visible:ring-2 ${selected === item.id ? 'bg-secondary text-high' : 'text-low'}`}
            onClick={() => setSelected(item.id)}
          >
            {drafts[item.id]?.name ?? item.name}
            {item.id === state.current_version_id
              ? ` · ${t('issueVersions.current')}`
              : ''}
            {drafts[item.id]?.saved === false ? ' *' : ''}
          </PrimaryButton>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-half px-base">
        <input
          aria-label={t('issueVersions.name')}
          className="rounded-sm border bg-primary px-half text-normal"
          value={draft.name}
          onChange={(event) => edit({ name: event.target.value })}
        />
        <PrimaryButton
          variant="tertiary"
          type="button"
          disabled={busy || draft.saved || !draft.name.trim()}
          onClick={() =>
            void mutate(async () => {
              const captured = draft;
              const next = await request(
                `/${selected}`,
                {
                  expected_revision: captured.revision,
                  name: captured.name,
                  ...(captured.description !== (version.description ?? '')
                    ? { description: captured.description || null }
                    : {}),
                },
                'PATCH'
              );
              const saved = next.versions.find((item) => item.id === selected)!;
              setDrafts((previous) => {
                const latest = previous[selected];
                return {
                  ...previous,
                  [selected]: acknowledgeVersionSave(
                    latest,
                    captured,
                    saved.revision
                  ),
                };
              });
              accept(next);
            })
          }
        >
          {t('issueVersions.save')}
        </PrimaryButton>
        <span className="text-low" role="status">
          {busy && busyVersion === selected
            ? t('issueVersions.busy')
            : draft.saved
              ? t('issueVersions.saved')
              : t('issueVersions.unsaved')}
        </span>
        {selected !== state.current_version_id && (
          <PrimaryButton
            variant="tertiary"
            type="button"
            disabled={busy}
            onClick={() =>
              void mutate(async () => {
                accept(
                  await request(
                    '/current',
                    {
                      version_id: selected,
                      expected_current_revision: state.current_revision,
                      expected_version_revision: version.revision,
                    },
                    'POST'
                  )
                );
              })
            }
          >
            {t('issueVersions.makeCurrent')}
          </PrimaryButton>
        )}
      </div>
      <div className="flex flex-wrap gap-half px-base">
        <input
          aria-label={t('issueVersions.newName')}
          placeholder={t('issueVersions.newName')}
          className="rounded-sm border bg-primary px-half text-normal"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        {(['copy', 'empty'] as const).map((kind) => (
          <PrimaryButton
            variant="tertiary"
            type="button"
            key={kind}
            disabled={busy || !name.trim()}
            onClick={() =>
              void mutate(async () => {
                const next = await request(
                  '',
                  {
                    name,
                    ...(kind === 'copy' ? { source_version_id: selected } : {}),
                  },
                  'POST'
                );
                accept(next);
                setSelected(next.created_version_id ?? selected);
                setName('');
              })
            }
          >
            {kind === 'copy'
              ? t('issueVersions.copy')
              : t('issueVersions.empty')}
          </PrimaryButton>
        ))}
      </div>
      {error && (error.versionId === null || error.versionId === selected) && (
        <div className="px-base text-error" role="alert">
          {error.message}
          <PrimaryButton
            variant="tertiary"
            type="button"
            disabled={busy}
            onClick={() =>
              void mutate(async () => {
                const next = await request();
                accept(next);
                // Explicitly reconcile revisions while preserving every local draft.
                setDrafts((previous) =>
                  Object.fromEntries(
                    Object.entries(previous).map(([id, value]) => [
                      id,
                      {
                        ...value,
                        revision:
                          (id === selected
                            ? next.versions.find((item) => item.id === id)
                                ?.revision
                            : undefined) ?? value.revision,
                      },
                    ])
                  )
                );
              })
            }
          >
            {t('issueVersions.reload')}
          </PrimaryButton>
        </div>
      )}
      <WYSIWYGEditor
        {...editorProps}
        key={selected}
        contentKey={selected}
        value={draft.description}
        onChange={(description) => edit({ description })}
        saveStatus={draft.saved ? 'saved' : 'idle'}
      />
    </div>
  );
}
