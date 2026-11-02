import { useTranslation } from 'react-i18next';

export interface ExecutionApprovalDetailsProps {
  projectId?: string;
  workspaceId?: string;
  sessionId?: string;
  agentRunId: string;
  approvalId: string;
  toolName: string;
  operation: string;
  arguments: unknown;
}

function redact(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(redact);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [
        key,
        /token|secret|password/i.test(key) ||
        /^(authorization|cookie|api_key|apikey|private_key)$/i.test(key)
          ? '[REDACTED: credential field]'
          : redact(item),
      ])
    );
  }
  return value;
}

export function ExecutionApprovalDetails(props: ExecutionApprovalDetailsProps) {
  const { t } = useTranslation('tasks');
  const args = redact(props.arguments);
  return (
    <section
      className="border-b px-double py-base text-sm text-normal space-y-half"
      aria-label={t('approval.detailsTitle')}
    >
      <p className="font-medium break-words">{props.toolName}</p>
      <p>{t('approval.singleRequest')}</p>
      <details>
        <summary className="cursor-pointer focus-visible:outline">
          {t('approval.detailsTitle')}
        </summary>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-base break-all">
          {Object.entries({
            Project: props.projectId,
            Workspace: props.workspaceId,
            Session: props.sessionId,
            Run: props.agentRunId,
            Approval: props.approvalId,
          }).map(([key, value]) => (
            <div className="contents" key={key}>
              <dt>{key}</dt>
              <dd>{value ?? t('approval.unavailable')}</dd>
            </div>
          ))}
        </dl>
        <p className="break-words">
          {t('approval.invokeOperation', { operation: props.operation })}
        </p>
        <p>{t('approval.arguments')}</p>
        {args == null ? (
          <p>{t('approval.argumentsUnavailable')}</p>
        ) : (
          <>
            {typeof args === 'object' &&
              !Array.isArray(args) &&
              Object.keys(args).length === 0 && (
                <p>{t('approval.noArguments')}</p>
              )}
            <pre className="whitespace-pre-wrap break-all max-h-64 overflow-auto">
              {JSON.stringify(args, null, 2)}
            </pre>
          </>
        )}
        <p>{t('approval.redactionNotice')}</p>
        <p>{t('approval.originalExpiry')}</p>
      </details>
    </section>
  );
}
