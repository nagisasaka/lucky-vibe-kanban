import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { helpApi } from '@/shared/lib/helpApi';
import { getResolvedTheme, useTheme } from '@/shared/hooks/useTheme';
import { MarkdownPreview } from './MarkdownPreview';

const TERMINAL = new Set([
  'succeeded',
  'failed',
  'cancelled',
  'crashed',
  'audit_failed',
]);

/** Help shares the normal agent runtime; closing this panel does not cancel a run. */
export function InteractiveHelp({
  initialSessionId,
}: { initialSessionId?: string } = {}) {
  const { t } = useTranslation('common');
  const { theme } = useTheme();
  const client = useQueryClient();
  const [sessionId, setSessionId] = useState<string | undefined>(
    initialSessionId
  );
  const [prompt, setPrompt] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();
  const conversations = useQuery({
    queryKey: ['help-conversations'],
    queryFn: helpApi.list,
  });
  const conversation = useQuery({
    queryKey: ['help-conversation', sessionId],
    queryFn: () => helpApi.get(sessionId!),
    enabled: Boolean(sessionId),
    refetchInterval: (query) => {
      const runs = query.state.data?.runs;
      return runs?.some((run) => !TERMINAL.has(run.state.status))
        ? 1500
        : false;
    },
  });
  const active =
    conversation.data?.runs.some((run) => !TERMINAL.has(run.state.status)) ??
    false;
  const refresh = async () => {
    await client.invalidateQueries({ queryKey: ['help-conversations'] });
    await client.invalidateQueries({ queryKey: ['help-conversation'] });
  };
  const perform = async (action: () => Promise<void>) => {
    setPending(true);
    setError(undefined);
    try {
      await action();
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="space-y-base">
      <p className="text-low">{t('interactiveHelp.description')}</p>
      <label className="block">
        <span className="text-low">{t('interactiveHelp.conversation')}</span>
        <select
          className="block w-full rounded-sm border border-border bg-primary p-half text-normal"
          value={sessionId ?? ''}
          disabled={pending}
          onChange={(event) => setSessionId(event.target.value || undefined)}
        >
          <option value="">{t('interactiveHelp.new')}</option>
          {conversations.data?.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name ?? 'LVK Help'} ·{' '}
              {new Date(item.created_at).toLocaleString()} ·{' '}
              {item.id.slice(0, 8)}
            </option>
          ))}
        </select>
      </label>
      {(error || conversations.error || conversation.error) && (
        <p role="alert" className="text-error whitespace-pre-wrap">
          {error ?? String(conversation.error ?? conversations.error)}
        </p>
      )}
      {sessionId && conversation.isLoading && (
        <p role="status">{t('interactiveHelp.loading')}</p>
      )}
      {conversation.data && (
        <>
          <p className="text-low break-all">
            {t('interactiveHelp.currentSource')}:{' '}
            {conversation.data.source.build_revision} ·{' '}
            {conversation.data.source.commit}
          </p>
          <a
            className="text-brand underline"
            href={`/workspaces/${conversation.data.workspace_id}`}
            target="_blank"
            rel="noreferrer"
          >
            {t('interactiveHelp.openWorkspace')}
          </a>
          <div className="space-y-double" aria-live="polite">
            {conversation.data.runs.map((run) => (
              <article
                key={run.agent_run_id}
                className="border-b border-border pb-base"
              >
                {conversation.data.questions
                  ?.filter(
                    (question) => question.agent_run_id === run.agent_run_id
                  )
                  .map((question) => (
                    <div key={question.agent_run_id} className="space-y-half">
                      <p className="whitespace-pre-wrap text-high">
                        {question.prompt}
                      </p>
                      <p className="text-low break-all">
                        {t('interactiveHelp.answerSource')}:{' '}
                        {question.build_revision ??
                          t('interactiveHelp.unknownRevision')}{' '}
                        ·{' '}
                        {question.source_commit ??
                          t('interactiveHelp.unknownRevision')}
                      </p>
                    </div>
                  ))}
                <p className="text-low" role="status">
                  {t(`interactiveHelp.status.${run.state.status}`)}
                </p>
                {run.state.last_error && (
                  <p role="alert" className="text-error">
                    {run.state.last_error.message}
                  </p>
                )}
                {run.state.terminal_output?.content && (
                  <MarkdownPreview
                    content={run.state.terminal_output.content}
                    theme={getResolvedTheme(theme)}
                    density="compact"
                    disableImages
                  />
                )}
              </article>
            ))}
          </div>
        </>
      )}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!prompt.trim() || active || pending) return;
          void perform(async () => {
            const result = await helpApi.ask(prompt.trim(), sessionId);
            setSessionId(result.session_id);
            setPrompt('');
          });
        }}
        className="space-y-base"
      >
        <label className="block">
          <span>{t('interactiveHelp.question')}</span>
          <textarea
            className="block w-full rounded-sm border border-border bg-primary p-base text-normal focus:ring-2 focus:ring-brand"
            rows={3}
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            disabled={pending || active}
            maxLength={20000}
          />
        </label>
        <div className="flex flex-wrap gap-base">
          <PrimaryButton
            type="submit"
            disabled={
              pending ||
              active ||
              (Boolean(sessionId) && !conversation.data) ||
              !prompt.trim()
            }
            actionIcon={pending ? 'spinner' : undefined}
          >
            {t('interactiveHelp.ask')}
          </PrimaryButton>
          {sessionId && active && (
            <PrimaryButton
              type="button"
              variant="tertiary"
              disabled={pending}
              onClick={() => void perform(() => helpApi.cancel(sessionId))}
            >
              {t('interactiveHelp.cancel')}
            </PrimaryButton>
          )}
          {sessionId && conversation.data && !active && (
            <PrimaryButton
              type="button"
              variant="tertiary"
              disabled={pending}
              onClick={() =>
                void perform(async () => {
                  await helpApi.delete(sessionId);
                  setSessionId(undefined);
                })
              }
            >
              {t('interactiveHelp.delete')}
            </PrimaryButton>
          )}
        </div>
      </form>
      <p className="text-low">{t('interactiveHelp.retention')}</p>
    </div>
  );
}
