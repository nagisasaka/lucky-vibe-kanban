import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useDevServer } from '@/shared/hooks/useDevServer';
import { useLogsPanel } from '@/shared/hooks/useLogsPanel';
import { useLogStream } from '@/shared/hooks/useLogStream';
import { ScriptFixerDialog } from '@/shared/dialogs/scripts/ScriptFixerDialog';
import { getDevServerWorkingDir } from '@/shared/lib/devServerUtils';
import {
  detectDevServerUrl,
  externalDevServerUrl,
} from '@/shared/lib/devServerUrl';

function DevServerUrl({ processId }: { processId: string }) {
  const { t } = useTranslation('tasks');
  const { logs } = useLogStream(processId);
  const [override, setOverride] = useState<string | null>(null);
  const value = override ?? detectDevServerUrl(logs) ?? '';
  const url = externalDevServerUrl(value);
  return (
    <div className="flex flex-col gap-half">
      <div className="flex flex-wrap items-center gap-base">
        <input
          className="min-w-0 flex-1 rounded-sm border border-border bg-primary px-base py-half text-sm text-normal focus-visible:outline-brand"
          aria-label={t('devServer.url')}
          placeholder={t('devServer.url')}
          type="url"
          value={value}
          onChange={(event) => setOverride(event.target.value)}
        />
        <button
          type="button"
          className="text-brand hover:text-brand-hover disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
          disabled={!url}
          onClick={() => url && navigator.clipboard.writeText(url)}
        >
          {t('devServer.copyUrl')}
        </button>
        {url && (
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-brand hover:text-brand-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
          >
            {t('devServer.openBrowser')}
          </a>
        )}
      </div>
      <p className="text-xs text-low">{t('devServer.addressHint')}</p>
    </div>
  );
}

/** Managed controls live with logs, independently of any application browser. */
export function DevServerControlsContainer() {
  const { t } = useTranslation('tasks');
  const { workspaceId, repos } = useWorkspaceContext();
  const [error, setError] = useState(false);
  const {
    start,
    stop,
    isStarting,
    isStopping,
    runningDevServers,
    devServerProcesses,
  } = useDevServer(workspaceId, {
    onStartError: () => setError(true),
    onStopError: () => setError(true),
    onStartSuccess: () => setError(false),
    onStopSuccess: () => setError(false),
  });
  const { logsPanelContent, viewProcessInPanel } = useLogsPanel();
  if (!workspaceId) return null;
  const selected =
    devServerProcesses.find(
      (p) =>
        logsPanelContent?.type === 'process' &&
        p.id === logsPanelContent.processId
    ) ?? devServerProcesses[0];
  const running = runningDevServers.length > 0;
  const busy = isStarting || isStopping;
  return (
    <section
      aria-label={t('devServer.title')}
      className="shrink-0 border-b border-border bg-secondary p-base flex flex-col gap-base text-sm text-normal"
    >
      <div className="flex flex-wrap items-center gap-base">
        <span className="font-medium">{t('devServer.title')}</span>
        <span role="status">
          {isStarting
            ? t('devServer.starting')
            : isStopping
              ? t('devServer.stopping')
              : running
                ? t('devServer.running')
                : t('devServer.stopped')}
        </span>
        <button
          type="button"
          disabled={busy}
          onClick={() => (running ? stop() : start())}
          className="text-brand hover:text-brand-hover disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
        >
          {running ? t('devServer.stop') : t('devServer.start')}
        </button>
        <button
          type="button"
          className="text-brand hover:text-brand-hover disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
          disabled={repos.length === 0}
          onClick={() =>
            ScriptFixerDialog.show({
              scriptType: 'dev_server',
              repos,
              workspaceId,
              sessionId: selected?.session_id,
              initialRepoId: repos.length === 1 ? repos[0].id : undefined,
            })
          }
        >
          {t('devServer.editScript')}
        </button>
      </div>
      {error && (
        <p role="alert" className="text-error">
          {t('devServer.operationUnconfirmed')}
        </p>
      )}
      {devServerProcesses.length > 0 && (
        <div
          className="flex flex-wrap gap-base"
          aria-label={t('devServer.logs')}
        >
          {devServerProcesses.map((process) => (
            <button
              type="button"
              key={process.id}
              aria-pressed={
                logsPanelContent?.type === 'process' &&
                logsPanelContent.processId === process.id
              }
              onClick={() => viewProcessInPanel(process.id)}
              className="text-brand hover:text-brand-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
            >
              {getDevServerWorkingDir(process) ?? t('devServer.title')} ·{' '}
              {process.status === 'failed' ||
              (process.exit_code != null && process.exit_code !== 0n)
                ? t('devServer.failed')
                : process.status}
              {process.exit_code != null &&
                process.exit_code !== 0n &&
                t('devServer.exitCode', { code: String(process.exit_code) })}
            </button>
          ))}
        </div>
      )}
      {selected && <DevServerUrl key={selected.id} processId={selected.id} />}
    </section>
  );
}
