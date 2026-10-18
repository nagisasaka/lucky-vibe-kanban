import { useState } from 'react';
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
}: {
  issuer: string;
  currentClientId: string;
  disabled: boolean;
  onIssuerChange: (value: string) => void;
  onBusyChange: (value: boolean) => void;
  onConfigured: (result: Result) => void;
}) {
  const [token, setToken] = useState('');
  const [inventory, setInventory] = useState<Inventory>();
  const [inventoryIssuer, setInventoryIssuer] = useState('');
  const [connection, setConnection] = useState('');
  const [web, setWeb] = useState('');
  const [chat, setChat] = useState('');
  const [callback, setCallback] = useState('');
  const [mcp, setMcp] = useState(false);
  const [refresh, setRefresh] = useState(false);
  const [preview, setPreview] = useState<{
    plan: Plan;
    confirmation: string;
    input: string;
  }>();
  const [result, setResult] = useState<Result>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
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
  async function run(step: 'inspect' | 'preview' | 'apply') {
    setBusy(true);
    onBusyChange(true);
    setError('');
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
        setResult(next);
        onConfigured(next);
      }
    } catch (e) {
      const message =
        e instanceof Error ? e.message : 'Auth0の設定に失敗しました';
      setError(
        step === 'apply'
          ? `${message} Auth0側に一部の変更が適用されている可能性があります。トークンを再入力して変更内容を確認し直してください。`
          : message
      );
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
        Auth0を自動セットアップ
      </summary>
      <div className="mt-base space-y-base">
        <p>
          既存のGitHub／Google接続を使って、LVK用アプリを作成または設定します。GitHub／Google側のOAuthアプリは先にAuth0へ登録してください。
        </p>
        <details>
          <summary className="cursor-pointer">
            管理トークンの準備と必要な権限
          </summary>
          <div className="space-y-base py-base text-low">
            <p>
              Auth0でManagement APIを許可した管理用Machine to
              Machineアプリを用意し、短い有効期限のアクセストークンを取得してください。ログイン用Client
              Secretとは別の値です。
            </p>
            <a
              className="underline"
              href="https://auth0.com/docs/secure/tokens/access-tokens/management-api-access-tokens/get-management-api-access-tokens-for-production"
              target="_blank"
              rel="noreferrer"
            >
              Auth0公式のトークン取得手順
            </a>
            <p className="break-words">
              必要な権限：read:clients、read:client_keys、create:clients、update:clients、read:connections、update:connections、read:tenant_settings、update:tenant_settings。MCPも設定する場合：read:resource_servers、create:resource_servers、update:resource_servers。
            </p>
            <p>
              トークンはサーバーに保存せず、適用の成否にかかわらず入力欄から消去します。有効期限まではAuth0側で有効です。
            </p>
          </div>
        </details>
        {error && (
          <p role="alert" className="text-error">
            {error}
          </p>
        )}
        <fieldset disabled={busy || disabled} className="min-w-0 space-y-base">
          <label className="block">
            設定先のAuth0 issuer（末尾 / を含む）
            <input
              className={field}
              type="url"
              placeholder="https://your-tenant.jp.auth0.com/"
              value={issuer}
              onChange={(e) => onIssuerChange(e.target.value)}
            />
          </label>
          <label className="block">
            Management APIアクセストークン
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
            接続とアプリを取得
          </button>
          {ready && (
            <>
              {!inventory.connections.length && (
                <p>
                  GitHubまたはGoogle接続がありません。Auth0でソーシャル接続を作成してから再取得してください。
                </p>
              )}
              <label className="block">
                ログインに使う接続
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
                LVK Web用アプリ
                <select
                  className={field}
                  value={web}
                  onChange={(e) => setWeb(e.target.value)}
                >
                  <option value="">
                    自動作成（このLVK用に作成済みなら再利用）
                  </option>
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
                ChatGPT向けMCPも設定する
              </label>
              {mcp && (
                <>
                  <label className="block">
                    ChatGPT用アプリ（Web用とは別）
                    <select
                      className={field}
                      value={chat}
                      onChange={(e) => setChat(e.target.value)}
                    >
                      <option value="">自動作成（作成済みなら再利用）</option>
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
                    ChatGPTに表示された正確なCallback URL
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
                    更新トークンを設定する（ローテーション有効・最大30日・未使用7日）
                  </label>
                </>
              )}
              <button
                className={button}
                type="button"
                disabled={!token || !connection || (mcp && !callback)}
                onClick={() => void run('preview')}
              >
                変更内容を確認
              </button>
            </>
          )}
          {validPreview && (
            <div className="space-y-base rounded border p-base">
              <h3 className="text-high">Auth0への適用内容（5分間有効）</h3>
              <ul className="list-disc space-y-half pl-double">
                <li>
                  Web用アプリ：
                  {validPreview.plan.web.id
                    ? `既存 ${validPreview.plan.web.id} を更新`
                    : '新規作成'}
                </li>
                {validPreview.plan.chatgpt && (
                  <li>
                    ChatGPT用アプリ：
                    {validPreview.plan.chatgpt.id
                      ? `既存 ${validPreview.plan.chatgpt.id} を更新`
                      : '新規作成'}
                    。MCP APIと権限も設定します。
                  </li>
                )}
                <li>
                  対象アプリは選択したソーシャル接続だけを有効にし、Authorization
                  Code・RS256・Client Secret Basicに設定します。既存Callback
                  URLは保持します。
                </li>
                {Object.keys(validPreview.plan.tenant).length > 0 && (
                  <li>
                    テナント共通のResource Parameter Compatibility
                    Profile／Include
                    Issuer設定を有効にします。他のアプリにも適用されます。
                  </li>
                )}
              </ul>
              <details>
                <summary className="cursor-pointer">設定値の詳細</summary>
                <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">
                  {JSON.stringify(validPreview.plan, null, 2)}
                </pre>
              </details>
              <p>
                LVKの認証方式はまだ切り替わりません。適用後、下のフォームでログイン試験を行ってください。
              </p>
              <button
                className={button}
                type="button"
                disabled={!token}
                onClick={() => void run('apply')}
              >
                この内容をAuth0へ適用
              </button>
            </div>
          )}
        </fieldset>
        {busy && <p role="status">Auth0を確認・設定しています…</p>}
        {result && (
          <div className="space-y-base rounded border p-base" role="status">
            <p>
              Auth0の設定が完了しました。Web用の情報を下のログイン試験フォームへ入力しました。
            </p>
            {result.chatgpt && (
              <>
                <p>
                  ChatGPT用の接続情報を保存しました。先にログイン試験とAuth0の有効化を完了し、ログイン後に認証設定の「ChatGPTへのMCP接続」からコピーしてください。
                </p>
                <p className="break-all">MCP URL：{result.resource}</p>
                <p>
                  要求スコープは必要なものだけ選んでください：lvk:read /
                  lvk:write / lvk:merge
                  {refresh
                    ? '。更新する場合は offline_access も指定します。'
                    : ''}
                </p>
              </>
            )}
            <button
              className={button}
              type="button"
              onClick={() => setResult(undefined)}
            >
              結果の表示を消去
            </button>
          </div>
        )}
      </div>
    </details>
  );
}
