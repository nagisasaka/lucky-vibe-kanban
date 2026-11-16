import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from '@tanstack/react-router';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { TransferArchiveText } from '@/shared/components/TransferArchiveText';
import { useShape } from '@/shared/integrations/electric/hooks';
import { refreshShapeFallback } from '@/shared/lib/electric/collections';
import { PROJECTS_SHAPE } from 'shared/remote-types';
import { LOCAL_ORG_ID } from '@/shared/lib/localIdentity';
import { repoApi } from '@/shared/lib/api';
import {
  projectTransferApi,
  type ImportedHistory,
  type TransferPreview,
  type TransferReport,
} from '@/shared/lib/projectTransferApi';

const importProjectParams = { organization_id: LOCAL_ORG_ID };

export function ProjectTransferPanel({ projectId }: { projectId?: string }) {
  const { t } = useTranslation('common');
  const projectList = useShape(PROJECTS_SHAPE, importProjectParams);
  const [importedProjectId, setImportedProjectId] = useState<string | null>(
    null
  );
  const [includeHistory, setIncludeHistory] = useState(false);
  const [bytes, setBytes] = useState<Uint8Array | null>(null);
  const [previewBytes, setPreviewBytes] = useState<Uint8Array | null>(null);
  const [preview, setPreview] = useState<TransferPreview | null>(null);
  const [key, setKey] = useState('');
  const [repoMap, setRepoMap] = useState<Record<string, string>>({});
  const [repos, setRepos] = useState<{ id: string; name: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [report, setReport] = useState<TransferReport | null>(null);
  const [history, setHistory] = useState<ImportedHistory[] | null>(null);
  const selection = useRef(0);
  useEffect(() => {
    if (!projectId) return;
    let active = true;
    projectTransferApi
      .report(projectId)
      .then((value) => {
        if (active) setReport(value);
      })
      .catch((e) => {
        if (active) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      active = false;
    };
  }, [projectId]);
  async function run(action: () => Promise<void>) {
    setError('');
    setBusy(true);
    try {
      await action();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <CollapsibleSectionHeader
      className="!h-auto"
      title={t(projectId ? 'projectTransfer.title' : 'projectTransfer.import')}
      persistKey="project-transfer"
    >
      <div className="space-y-base p-base text-xs text-normal">
        <p>{t('projectTransfer.scope')}</p>
        {projectId && (
          <>
            <label className="flex gap-half items-start">
              <input
                type="checkbox"
                checked={includeHistory}
                disabled={busy}
                onChange={(e) => setIncludeHistory(e.target.checked)}
              />
              {t('projectTransfer.optIn')}
            </label>
            <p className="text-low">{t('projectTransfer.historyScope')}</p>
            <PrimaryButton
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  if (!projectId) return;
                  const blob = await projectTransferApi.export(
                    projectId,
                    includeHistory
                  );
                  const url = URL.createObjectURL(blob);
                  try {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `project-${projectId}.sqlite`;
                    a.click();
                  } finally {
                    setTimeout(() => URL.revokeObjectURL(url), 1000);
                  }
                })
              }
            >
              {t('projectTransfer.export')}
            </PrimaryButton>
          </>
        )}
        <p>{t('projectTransfer.importScope')}</p>
        <label className="block">
          {t('projectTransfer.file')}
          <input
            className="block w-full"
            type="file"
            accept=".sqlite,.db,application/vnd.sqlite3"
            disabled={busy}
            onChange={(e) => {
              const file = e.target.files?.[0];
              const generation = ++selection.current;
              setBytes(null);
              setPreview(null);
              setPreviewBytes(null);
              setRepoMap({});
              setReport(null);
              setError('');
              if (file)
                void run(async () => {
                  if (file.size > 32 * 1024 * 1024)
                    throw new Error(t('projectTransfer.tooLarge'));
                  const selected = new Uint8Array(await file.arrayBuffer());
                  if (generation === selection.current) setBytes(selected);
                });
            }}
          />
        </label>
        <PrimaryButton
          variant="secondary"
          disabled={busy || !bytes}
          onClick={() =>
            void run(async () => {
              if (!bytes) return;
              const generation = selection.current;
              const [result, available] = await Promise.all([
                projectTransferApi.preview(bytes),
                repoApi.list(),
              ]);
              if (generation !== selection.current) return;
              setPreviewBytes(bytes);
              setPreview(result);
              setRepos(available);
            })
          }
        >
          {t('projectTransfer.preview')}
        </PrimaryButton>
        {preview && (
          <div className="space-y-base border-l pl-base">
            <p className="font-medium text-high">{preview.project_name}</p>
            <p>
              {t('projectTransfer.counts', {
                issues: preview.issue_count,
                versions: preview.version_count,
                comments: preview.comment_count,
                histories: preview.history_count ?? 0,
                manifests: preview.manifest_count ?? 0,
              })}
            </p>
            <p>{preview.coverage}</p>
            <p>{preview.history_omissions}</p>
            <p>
              {t('projectTransfer.external', {
                count: preview.external_reference_count,
              })}
            </p>
            <p>
              {t('projectTransfer.missing', {
                count: preview.missing_attachments.length,
              })}
            </p>
            <ul>
              {preview.missing_attachments.map((a, i) => (
                <li key={i}>{a.name}</li>
              ))}
            </ul>
            <label className="block">
              {t('projectTransfer.key')}
              <input
                className="block w-full rounded-sm border bg-primary p-half"
                value={key}
                disabled={busy}
                onChange={(e) => setKey(e.target.value)}
              />
            </label>
            {preview.repositories.map((repo) => (
              <label className="block" key={repo.source_id}>
                {repo.display_name}
                <select
                  className="block w-full rounded-sm border bg-primary p-half"
                  value={repoMap[repo.source_id] ?? ''}
                  disabled={busy}
                  onChange={(e) =>
                    setRepoMap((m) => ({
                      ...m,
                      [repo.source_id]: e.target.value,
                    }))
                  }
                >
                  <option value="">{t('projectTransfer.selectRepo')}</option>
                  {repos.map((r) => (
                    <option value={r.id} key={r.id}>
                      {r.name}
                    </option>
                  ))}
                </select>
              </label>
            ))}
            <PrimaryButton
              disabled={
                busy ||
                !bytes ||
                !previewBytes ||
                !preview.sha256 ||
                !key.trim() ||
                preview.repositories.some((r) => !repoMap[r.source_id])
              }
              onClick={() =>
                void run(async () => {
                  if (previewBytes) {
                    const receipt = await projectTransferApi.import(
                      previewBytes,
                      preview.sha256,
                      key.trim(),
                      repoMap
                    );
                    setReport(receipt);
                    setImportedProjectId(receipt.project_id);
                    setPreview(null);
                    setPreviewBytes(null);
                    setBytes(null);
                    try {
                      await refreshShapeFallback(
                        PROJECTS_SHAPE,
                        importProjectParams
                      );
                    } catch {
                      /* The receipt remains successful; show reload guidance until observed. */
                    }
                  }
                })
              }
            >
              {t('projectTransfer.import')}
            </PrimaryButton>
          </div>
        )}
        {report && (
          <div role="status">
            <p>
              {!importedProjectId ||
              projectList.data.some(
                (project) => project.id === importedProjectId
              ) ? (
                <Link
                  to="/projects/$projectId"
                  params={{ projectId: report.project_id }}
                  className="text-brand underline"
                >
                  {t('projectTransfer.imported')}
                </Link>
              ) : (
                <span>{t('projectTransfer.importedRefresh')}</span>
              )}
            </p>
            <p>
              {t('projectTransfer.missing', {
                count: report.missing_attachments.length,
              })}
            </p>
            <ul>
              {report.missing_attachments.map((a, i) => (
                <li key={i}>{a.name}</li>
              ))}
            </ul>
            <p>{report.history_omissions}</p>
          </div>
        )}
        {projectId && (
          <PrimaryButton
            variant="secondary"
            disabled={busy}
            onClick={() =>
              void run(async () =>
                setHistory(await projectTransferApi.history(projectId!))
              )
            }
          >
            {t('projectTransfer.history')}
          </PrimaryButton>
        )}
        {projectId && history && (
          <div className="space-y-base">
            <p>{t('projectTransfer.readOnly')}</p>
            {history.length === 0 && <p>{t('projectTransfer.empty')}</p>}
            {history.map((item) => (
              <details key={item.run_id} className="border-l pl-base">
                <summary className="cursor-pointer break-all">
                  {item.run_id} ({item.status})
                </summary>
                <Link
                  to="/projects/$projectId/issues/$issueId"
                  params={{ projectId, issueId: item.issue_id }}
                  className="text-brand underline"
                >
                  {t('projectTransfer.issue')}
                </Link>
                {item.messages.map((message, i) => (
                  <div className="my-base" key={i}>
                    <p className="font-medium">{message.role}</p>
                    <TransferArchiveText content={message.content} />
                  </div>
                ))}
                {item.manifests.map((manifest) => (
                  <details key={manifest.id}>
                    <summary className="cursor-pointer">
                      {t('projectTransfer.manifest')} {manifest.id}
                    </summary>
                    <dl className="space-y-half">
                      {(
                        [
                          'created_at',
                          'base_commit',
                          'source_commit',
                          'target_branch',
                          'changed_paths',
                        ] as const
                      ).map((field) => {
                        const value = manifest[field];
                        if (value == null) return null;
                        return (
                          <div key={field}>
                            <dt className="font-medium">
                              {t(`projectTransfer.manifestFields.${field}`)}
                            </dt>
                            <dd className="whitespace-pre-wrap break-all">
                              {Array.isArray(value) ? value.join('\n') : value}
                            </dd>
                          </div>
                        );
                      })}
                      {Object.entries(manifest.semantics).map(
                        ([field, value]) => (
                          <div key={field}>
                            <dt className="font-medium">{field}</dt>
                            <dd className="whitespace-pre-wrap break-words">
                              {typeof value === 'string'
                                ? value
                                : Array.isArray(value)
                                  ? value.map((item, index) => (
                                      <p key={index}>
                                        {typeof item === 'string'
                                          ? item
                                          : item && typeof item === 'object'
                                            ? Object.values(item)
                                                .filter(
                                                  (v) => typeof v === 'string'
                                                )
                                                .join(' — ')
                                            : ''}
                                      </p>
                                    ))
                                  : ''}
                            </dd>
                          </div>
                        )
                      )}
                    </dl>
                  </details>
                ))}
              </details>
            ))}
          </div>
        )}
        {error && (
          <p role="alert" className="text-error break-words">
            {error}
          </p>
        )}
      </div>
    </CollapsibleSectionHeader>
  );
}
