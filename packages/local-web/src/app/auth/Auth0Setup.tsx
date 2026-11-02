import {
  AccessRequestError,
  accessErrorNotice,
  toAccessRequestError,
} from '@/shared/lib/accessErrorMessage';
import { useTranslation } from 'react-i18next';
import { useEffect, useState } from 'react';
import { accessRequest } from './serverAccess';

interface Client {
  client_id: string;
  name: string;
}
interface Connection {
  id: string;
  name: string;
  strategy: string;
}
interface Inventory {
  clients: Client[];
  connections: Connection[];
}
interface ObjectPlan {
  id: string | null;
  settings: Record<string, unknown>;
}
interface Plan {
  web: ObjectPlan;
  chatgpt: ObjectPlan | null;
  resource: ObjectPlan | null;
  tenant: Record<string, unknown>;
}
interface Credentials {
  client_id: string;
  client_secret: string;
}
interface Result {
  issuer: string;
  mcp_enabled: boolean;
  resource: string;
  web: Credentials;
  chatgpt?: Credentials;
}
const field = 'min-w-0 w-full rounded border bg-secondary p-base text-normal';
const button =
  'rounded border bg-secondary px-double py-base text-high hover:bg-panel disabled:opacity-50';

export function Auth0Setup({
  issuer,
  currentClientId,
  disabled,
  onIssuerChange,
  onBusyChange,
  onConfigured,
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  issuer: string;
  currentClientId: string;
  disabled: boolean;
  onIssuerChange: (value: string) => void;
  onBusyChange: (value: boolean) => void;
  onConfigured: (result: Result) => void;
}) {
  const { t } = useTranslation('settings');
  const [token, setToken] = useState('');
  const [inventory, setInventory] = useState<Inventory>();
  const [inventoryIssuer, setInventoryIssuer] = useState('');
  const [connection, setConnection] = useState('');
  const [web, setWeb] = useState('');
  const [chat, setChat] = useState('');
  const [callback, setCallback] = useState('');
  const [mcp, setMcp] = useState(false);
  const [refresh, setRefresh] = useState(true);
  const [preview, setPreview] = useState<{
    plan: Plan;
    confirmation: string;
    input: string;
  }>();
  const [result, setResult] = useState<Result & { refresh_tokens: boolean }>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{
    cause: AccessRequestError;
    apply: boolean;
  } | null>(null);
  const options = {
    issuer,
    connection_id: connection,
    web_client_id: web,
    chatgpt_client_id: chat,
    chatgpt_callback: callback,
    mcp_enabled: mcp,
    refresh_tokens: refresh,
  };
  const inputKey = JSON.stringify(options);
  const validPreview = preview?.input === inputKey ? preview : undefined;
  const ready = inventory && inventoryIssuer === issuer;
  useEffect(() => {
    onDirtyChange?.(
      Boolean(
        !result &&
          (token || connection || web || chat || callback || mcp || !refresh)
      )
    );
    return () => onDirtyChange?.(false);
  }, [
    token,
    connection,
    web,
    chat,
    callback,
    mcp,
    refresh,
    result,
    onDirtyChange,
  ]);
  async function run(step: 'inspect' | 'preview' | 'apply') {
    setBusy(true);
    onBusyChange(true);
    setError(null);
    setResult(undefined);
    try {
      const input = { ...options, management_token: token, step };
      if (step === 'inspect') {
        setPreview(undefined);
        setInventory(undefined);
        const next = await accessRequest<Inventory>('auth0/provision', input);
        setInventory(next);
        setInventoryIssuer(issuer);
        setConnection(
          next.connections.find((c) => c.strategy === 'github')?.id ??
            next.connections[0]?.id ??
            ''
        );
        setWeb(
          next.clients.some((c) => c.client_id === currentClientId)
            ? currentClientId
            : ''
        );
        setChat('');
      } else if (step === 'preview') {
        setPreview(undefined);
        const next = await accessRequest<{ plan: Plan; confirmation: string }>(
          'auth0/provision',
          input
        );
        setPreview({ ...next, input: inputKey });
      } else if (validPreview) {
        const next = await accessRequest<Result>('auth0/provision', {
          ...input,
          confirmation: validPreview.confirmation,
        });
        setResult({ ...next, refresh_tokens: refresh });
        onConfigured(next);
      }
    } catch (e) {
      setError({
        cause: toAccessRequestError(e, 'auth.setup.failed'),
        apply: step === 'apply',
      });
    } finally {
      if (step === 'apply') {
        setToken('');
        setPreview(undefined);
      }
      setBusy(false);
      onBusyChange(false);
    }
  }
  return (
    <details className="rounded border p-base">
      <summary className="cursor-pointer text-high">
        {t('auth.setup.title')}
      </summary>
      <div className="mt-base space-y-base">
        <p>{t('auth.setup.description')}</p>
        <details>
          <summary className="cursor-pointer">
            {t('auth.setup.tokenHelp')}
          </summary>
          <div className="space-y-base py-base text-low">
            <p>{t('auth.setup.tokenDescription')}</p>
            <a
              className="underline"
              href="https://auth0.com/docs/secure/tokens/access-tokens/management-api-access-tokens/get-management-api-access-tokens-for-production"
              target="_blank"
              rel="noreferrer"
            >
              {t('auth.setup.tokenGuide')}
            </a>
            <p className="break-words">{t('auth.setup.permissions')}</p>
            <p>{t('auth.setup.tokenLifetime')}</p>
          </div>
        </details>
        {error && (
          <p role="alert" className="text-error">
            {error.apply
              ? t('auth.setup.applyUncertain', {
                  message: accessErrorNotice(error.cause),
                })
              : accessErrorNotice(error.cause)}
          </p>
        )}
        <fieldset disabled={busy || disabled} className="min-w-0 space-y-base">
          <label className="block">
            {t('auth.setup.issuer')}
            <input
              className={field}
              type="url"
              placeholder="https://your-tenant.jp.auth0.com/"
              value={issuer}
              onChange={(e) => onIssuerChange(e.target.value)}
            />
          </label>
          <label className="block">
            {t('auth.setup.token')}
            <input
              className={field}
              type="password"
              autoComplete="off"
              value={token}
              onChange={(e) => setToken(e.target.value)}
            />
          </label>
          <button
            type="button"
            className={button}
            disabled={!issuer || !token}
            onClick={() => void run('inspect')}
          >
            {t('auth.setup.inspect')}
          </button>
          {ready && (
            <>
              {!inventory.connections.length && (
                <p>{t('auth.setup.noConnections')}</p>
              )}
              <label className="block">
                {t('auth.setup.connection')}
                <select
                  className={field}
                  value={connection}
                  onChange={(e) => setConnection(e.target.value)}
                >
                  {inventory.connections.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name} ({c.strategy})
                    </option>
                  ))}
                </select>
              </label>
              <label className="block">
                {t('auth.setup.webApp')}
                <select
                  className={field}
                  value={web}
                  onChange={(e) => setWeb(e.target.value)}
                >
                  <option value="">{t('auth.setup.automaticWeb')}</option>
                  {inventory.clients.map((c) => (
                    <option key={c.client_id} value={c.client_id}>
                      {c.name} ({c.client_id})
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex gap-base">
                <input
                  type="checkbox"
                  checked={mcp}
                  onChange={(e) => setMcp(e.target.checked)}
                />
                {t('auth.setup.enableMcp')}
              </label>
              {mcp && (
                <>
                  <label className="block">
                    {t('auth.setup.chatgptApp')}
                    <select
                      className={field}
                      value={chat}
                      onChange={(e) => setChat(e.target.value)}
                    >
                      <option value="">
                        {t('auth.setup.automaticChatgpt')}
                      </option>
                      {inventory.clients
                        .filter((c) => c.client_id !== web)
                        .map((c) => (
                          <option key={c.client_id} value={c.client_id}>
                            {c.name} ({c.client_id})
                          </option>
                        ))}
                    </select>
                  </label>
                  <label className="block">
                    {t('auth.setup.callback')}
                    <input
                      className={field}
                      type="url"
                      value={callback}
                      onChange={(e) => setCallback(e.target.value)}
                    />
                  </label>
                  <label className="flex gap-base">
                    <input
                      type="checkbox"
                      checked={refresh}
                      onChange={(e) => setRefresh(e.target.checked)}
                    />
                    {t('auth.setup.refreshTokens')}
                  </label>
                </>
              )}
              <button
                className={button}
                type="button"
                disabled={!token || !connection || (mcp && !callback)}
                onClick={() => void run('preview')}
              >
                {t('auth.setup.preview')}
              </button>
            </>
          )}
          {validPreview && (
            <div className="space-y-base rounded border p-base">
              <h3 className="text-high">{t('auth.setup.previewTitle')}</h3>
              <ul className="list-disc space-y-half pl-double">
                <li>
                  {t('auth.setup.webPlan')}
                  {validPreview.plan.web.id
                    ? t('auth.setup.updateExisting', {
                        id: validPreview.plan.web.id,
                      })
                    : t('auth.setup.createNew')}
                </li>
                {validPreview.plan.chatgpt && (
                  <li>
                    {t('auth.setup.chatgptPlan')}
                    {validPreview.plan.chatgpt.id
                      ? t('auth.setup.updateExisting', {
                          id: validPreview.plan.chatgpt.id,
                        })
                      : t('auth.setup.createNew')}
                    {t('auth.setup.mcpPlan')}
                  </li>
                )}
                <li>{t('auth.setup.appPlan')}</li>
                {Object.keys(validPreview.plan.tenant).length > 0 && (
                  <li>{t('auth.setup.tenantPlan')}</li>
                )}
              </ul>
              <details>
                <summary className="cursor-pointer">
                  {t('auth.setup.details')}
                </summary>
                <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">
                  {JSON.stringify(validPreview.plan, null, 2)}
                </pre>
              </details>
              <p>{t('auth.setup.activateHint')}</p>
              <button
                className={button}
                type="button"
                disabled={!token}
                onClick={() => void run('apply')}
              >
                {t('auth.setup.apply')}
              </button>
            </div>
          )}
        </fieldset>
        {busy && <p role="status">{t('auth.setup.busy')}</p>}
        {result && (
          <div className="space-y-base rounded border p-base" role="status">
            <p>{t('auth.setup.success')}</p>
            {result.chatgpt && (
              <>
                <p>{t('auth.setup.chatgptSaved')}</p>
                <p className="break-all">
                  {t('auth.setup.mcpUrl', { url: result.resource })}
                </p>
                <p>
                  {t('auth.setup.scopes')}
                  {result.refresh_tokens ? t('auth.setup.offlineAccess') : ''}
                </p>
              </>
            )}
            <button
              className={button}
              type="button"
              onClick={() => setResult(undefined)}
            >
              {t('auth.setup.clear')}
            </button>
          </div>
        )}
      </div>
    </details>
  );
}
