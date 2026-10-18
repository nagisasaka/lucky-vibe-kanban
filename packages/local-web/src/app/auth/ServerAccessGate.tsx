import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { Auth0Setup } from './Auth0Setup';
import { ChatGptCredentials } from './ChatGptCredentials';
import {
  accessRequest,
  loadAccessStatus,
  type AccessStatus,
} from './serverAccess';

interface Member {
  id: string;
  issuer: string;
  subject: string;
  display_name: string;
  state: string;
  role: string;
}
interface Configuration {
  public_url: string;
  callback_url: string;
  logout_url: string;
  resource: string;
  scopes: string[];
  current: {
    mode?: string;
    issuer?: string;
    client_id?: string;
    username?: string;
    mcp_enabled?: boolean;
    secret_configured?: boolean;
  };
}
interface Confirmation {
  ready: boolean;
  identity?: Member;
}
const field = 'min-w-0 w-full rounded border bg-secondary p-base text-normal';
const button =
  'shrink-0 whitespace-nowrap rounded border bg-secondary px-double py-base text-high hover:bg-panel disabled:opacity-50';

function CopyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-half">
      <label className="text-low">{label}</label>
      <div className="flex gap-base">
        <input className={field} readOnly value={value} aria-label={label} />
        <button
          className={button}
          type="button"
          onClick={() => void navigator.clipboard.writeText(value)}
        >
          コピー
        </button>
      </div>
    </div>
  );
}

export function ServerAccessGate({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AccessStatus>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [provisioningBusy, setProvisioningBusy] = useState(false);
  const [key, setKey] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [issuer, setIssuer] = useState('');
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [mcp, setMcp] = useState(false);
  const [credentialsVersion, setCredentialsVersion] = useState(0);
  const [mode, setMode] = useState('auth0');
  const [configuration, setConfiguration] = useState<Configuration>();
  const [confirmation, setConfirmation] = useState<Confirmation>();
  const [members, setMembers] = useState<Member[]>([]);
  const [audit, setAudit] = useState<
    Array<{
      id: number;
      at: number;
      actor: string | null;
      action: string;
      outcome: string;
    }>
  >([]);
  const view = new URLSearchParams(window.location.search).get('access');
  const settings = status?.limited || view === 'settings' || view === 'confirm';
  const recovery = view === 'recovery';
  const refresh = useCallback(async () => {
    try {
      setStatus(await loadAccessStatus());
    } catch (e) {
      setError(e instanceof Error ? e.message : '認証状態を取得できません');
    }
  }, []);
  useEffect(() => {
    void refresh();
    window.addEventListener('lvk-access-expired', refresh);
    return () => window.removeEventListener('lvk-access-expired', refresh);
  }, [refresh]);
  useEffect(() => {
    if (!settings || !status?.management) return;
    void Promise.all([
      accessRequest<Configuration>('configuration'),
      accessRequest<Confirmation>('confirmation'),
    ])
      .then(([config, confirmation]) => {
        setConfiguration(config);
        setConfirmation(confirmation);
        setIssuer(config.current.issuer ?? '');
        setClientId(config.current.client_id ?? '');
        setMcp(config.current.mcp_enabled ?? false);
        setUsername(config.current.username ?? '');
        setMode(
          status.forced_mode ??
            (config.current.mode === 'basic' ? 'basic' : 'auth0')
        );
      })
      .catch((e: Error) => setError(e.message));
    if (status.authenticated && status.mode !== 'local') {
      void accessRequest<{ members: Member[] }>('members')
        .then((r) => setMembers(r.members))
        .catch((e: Error) => setError(e.message));
      void accessRequest<{ events: typeof audit }>('audit')
        .then((r) => setAudit(r.events))
        .catch((e: Error) => setError(e.message));
    }
  }, [settings, status]);
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (e) {
      setError(e instanceof Error ? e.message : '処理に失敗しました');
    } finally {
      setBusy(false);
    }
  }
  async function login() {
    await run(async () => {
      const result = await accessRequest<{ authorize_url: string }>('login');
      window.location.assign(result.authorize_url);
    });
  }
  if (status?.authenticated && !settings && !recovery) {
    return (
      <>
        {children}
        {status.mode !== 'local' && (
          <div className="new-design fixed bottom-base right-base z-50 flex gap-base rounded border bg-panel p-half text-normal">
            {status.user?.admin && (
              <a href="/?access=settings">認証・メンバー</a>
            )}
            <button
              onClick={() =>
                void run(async () => {
                  await accessRequest('logout', {});
                  window.location.assign('/');
                })
              }
            >
              ログアウト
            </button>
          </div>
        )}
      </>
    );
  }
  return (
    <main className="new-design min-h-screen overflow-auto bg-primary p-double text-base text-normal">
      <div className="mx-auto max-w-2xl space-y-double rounded border bg-panel p-double">
        <h1 className="text-xl text-high">
          LVK {settings ? '認証とメンバー管理' : 'ログイン・初期設定'}
        </h1>
        {error && (
          <p role="alert" className="text-error">
            {error}
          </p>
        )}
        {!status && <p>認証状態を確認しています…</p>}
        {status?.configuration_error && (
          <p>
            認証設定を読み込めないため保護を維持しています。管理者は復旧キーで設定し直してください。
          </p>
        )}
        {status?.mode === 'locked' && !status.limited && (
          <p>
            初期設定が完了していません。サーバー管理者が発行したキーで設定を開始してください。
          </p>
        )}
        {(recovery ||
          (status &&
            !status.authenticated &&
            !status.limited &&
            status.mode === 'locked') ||
          (settings && status?.mode === 'local' && !configuration)) && (
          <form
            className="space-y-base"
            onSubmit={(event) => {
              event.preventDefault();
              void run(async () => {
                await accessRequest(
                  'setup',
                  { key },
                  { 'x-lvk-csrf': 'setup' }
                );
                setKey('');
                window.location.assign('/?access=settings');
              });
            }}
          >
            <p>サーバーで実行するコマンド（キーは15分間有効・一度限り）：</p>
            <code className="block overflow-auto bg-secondary p-base">
              docker compose exec lvk server{' '}
              {recovery ? 'recovery-token' : 'setup-token'}
            </code>
            <label className="block">
              {recovery ? '復旧キー' : 'セットアップキー'}
              <input
                className={field}
                type="password"
                autoComplete="off"
                required
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
            </label>
            <button className={button} disabled={busy || provisioningBusy}>
              設定を開始
            </button>
            <p className="text-low">
              このブラウザだけに、認証設定用の期限付きセッションを発行します。
            </p>
          </form>
        )}
        {status?.mode === 'basic' &&
          !status.authenticated &&
          !recovery &&
          !status.limited && (
            <form
              className="space-y-base"
              onSubmit={(event) => {
                event.preventDefault();
                void run(async () => {
                  const encoded = btoa(
                    Array.from(
                      new TextEncoder().encode(`${username}:${password}`),
                      (c) => String.fromCharCode(c)
                    ).join('')
                  );
                  await accessRequest(
                    'basic-login',
                    {},
                    { authorization: `Basic ${encoded}`, 'x-lvk-csrf': 'basic' }
                  );
                  setPassword('');
                  await refresh();
                });
              }}
            >
              <label className="block">
                ユーザー名
                <input
                  className={field}
                  autoComplete="username"
                  required
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                />
              </label>
              <label className="block">
                パスワード
                <input
                  className={field}
                  type="password"
                  autoComplete="current-password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </label>
              <button className={button} disabled={busy || provisioningBusy}>
                ログイン
              </button>
            </form>
          )}
        {status?.mode === 'auth0' &&
          !status.authenticated &&
          !recovery &&
          !status.limited && (
            <div className="space-y-base">
              {view === 'pending' && (
                <p>
                  ログインは確認できました。LVK管理者のメンバー承認が必要です。無効化されている場合も管理者へ連絡してください。
                </p>
              )}
              <button
                className={button}
                disabled={busy || provisioningBusy}
                onClick={() => void login()}
              >
                Auth0でログイン
              </button>
              <p>
                Auth0の画面でGoogle／GitHub等の設定済みログイン方法を選びます。
              </p>
            </div>
          )}
        {settings && configuration && status?.management && (
          <>
            <CopyValue
              label="LVK公開URL（配置設定）"
              value={configuration.public_url || 'Local'}
            />
            <label className="block">
              認証方式
              <select
                aria-label="認証方式"
                className={field}
                value={mode}
                disabled={
                  Boolean(status.forced_mode) || busy || provisioningBusy
                }
                onChange={(e) => setMode(e.target.value)}
              >
                {status.deployment === 'local' && (
                  <option value="local">Local（ローカル無認証）</option>
                )}
                <option value="basic">Basic（単一所有者）</option>
                <option value="auth0">Auth0（承認されたメンバー）</option>
              </select>
            </label>
            {status.forced_mode && (
              <p>認証方式はLVK_AUTH_MODEで固定されています。</p>
            )}
            {mode === 'local' && (
              <button
                className={button}
                disabled={busy || provisioningBusy}
                onClick={() =>
                  void run(async () => {
                    await accessRequest('local', {});
                    window.location.assign('/');
                  })
                }
              >
                Localを有効化
              </button>
            )}
            {mode === 'basic' && (
              <form
                className="space-y-base"
                onSubmit={(event) => {
                  event.preventDefault();
                  void run(async () => {
                    await accessRequest('basic', { username, password });
                    setPassword('');
                    window.location.assign('/');
                  });
                }}
              >
                <p>
                  HTTPS上で単一所有者として利用します。リモートHTTP
                  MCPとMFAは提供しません。
                </p>
                <label className="block">
                  ユーザー名
                  <input
                    className={field}
                    required
                    value={username}
                    autoComplete="username"
                    onChange={(e) => setUsername(e.target.value)}
                  />
                </label>
                <label className="block">
                  新しいパスワード（12文字以上）
                  <input
                    className={field}
                    type="password"
                    autoComplete="new-password"
                    minLength={12}
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                </label>
                <button className={button} disabled={busy || provisioningBusy}>
                  Basicを設定して再ログイン
                </button>
              </form>
            )}
            {mode === 'auth0' && (
              <div className="space-y-double">
                <p>
                  ご自身のAuth0テナントとGitHub／Google接続を用意します。
                  アプリは自動セットアップで作成するか、手動で設定した情報を入力してください。
                </p>
                <Auth0Setup
                  issuer={issuer}
                  currentClientId={clientId}
                  disabled={busy || provisioningBusy}
                  onBusyChange={setProvisioningBusy}
                  onIssuerChange={setIssuer}
                  onConfigured={(result) => {
                    setIssuer(result.issuer);
                    setClientId(result.web.client_id);
                    setClientSecret(result.web.client_secret);
                    setMcp(result.mcp_enabled);
                    setCredentialsVersion((version) => version + 1);
                    setConfirmation(undefined);
                  }}
                />
                <CopyValue
                  label="WebアプリのAllowed Callback URL"
                  value={configuration.callback_url}
                />
                <CopyValue
                  label="Allowed Logout URL"
                  value={configuration.logout_url}
                />
                <form
                  className="space-y-base"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void run(async () => {
                      const result = await accessRequest<{
                        authorize_url: string;
                      }>('auth0/test', {
                        issuer,
                        client_id: clientId,
                        client_secret: clientSecret,
                        mcp_enabled: mcp,
                      });
                      setClientSecret('');
                      window.location.assign(result.authorize_url);
                    });
                  }}
                >
                  <label className="block">
                    Auth0 issuer（末尾 / を含む）
                    <input
                      className={field}
                      type="url"
                      placeholder="https://your-tenant.jp.auth0.com/"
                      required
                      value={issuer}
                      onChange={(e) => setIssuer(e.target.value)}
                    />
                  </label>
                  <label className="block">
                    Web用Client ID
                    <input
                      className={field}
                      required
                      value={clientId}
                      onChange={(e) => setClientId(e.target.value)}
                    />
                  </label>
                  <label className="block">
                    Web用Client Secret
                    <input
                      className={field}
                      type="password"
                      autoComplete="off"
                      required={!configuration.current.secret_configured}
                      placeholder={
                        configuration.current.secret_configured
                          ? '未入力なら現在のSecretを保持'
                          : ''
                      }
                      value={clientSecret}
                      onChange={(e) => setClientSecret(e.target.value)}
                    />
                  </label>
                  <label className="flex gap-base">
                    <input
                      type="checkbox"
                      checked={mcp}
                      onChange={(e) => setMcp(e.target.checked)}
                    />
                    OAuthで保護したリモートHTTP MCPを有効化
                  </label>
                  <p className="text-low">
                    Discovery／JWKSとPKCE
                    S256の公開設定を確認してからAuth0へ移動します。現行の認証方式は、ログイン試験後に有効化するまで維持されます。
                  </p>
                  <button
                    className={button}
                    disabled={busy || provisioningBusy}
                  >
                    設定をチェックしてログイン試験
                  </button>
                </form>
                {confirmation?.ready && confirmation.identity && (
                  <div className="space-y-base rounded border p-base">
                    <h2 className="text-lg text-high">Webログイン試験に成功</h2>
                    <p>次の本人識別情報をLVK管理者として確定します。</p>
                    <p>{confirmation.identity.display_name}</p>
                    <code className="block break-all">
                      {confirmation.identity.issuer} /{' '}
                      {confirmation.identity.subject}
                    </code>
                    <button
                      className={button}
                      disabled={busy || provisioningBusy}
                      onClick={() =>
                        void run(async () => {
                          await accessRequest('activate', {});
                          window.location.assign('/');
                        })
                      }
                    >
                      この管理者でAuth0を有効化
                    </button>
                  </div>
                )}
                <h2 className="text-lg text-high">ChatGPTへのMCP接続</h2>
                <ChatGptCredentials
                  key={`${issuer}:${clientId}:${credentialsVersion}`}
                  issuer={issuer}
                  webClientId={clientId}
                  disabled={busy || provisioningBusy}
                />
                <p>
                  自動セットアップ済みなら、Auth0での作成・設定を繰り返す必要はありません。
                  ChatGPTへMCP URLと専用Client
                  ID・Secretを登録し、接続を確認してください。
                  以下は手動で設定する場合の手順です。
                </p>
                <CopyValue
                  label="MCP URL / Auth0 API Identifier / Resource / Audience"
                  value={configuration.resource}
                />
                <CopyValue
                  label="API Permissions（必要な操作だけ許可）"
                  value={configuration.scopes.join(' ')}
                />
                <ol className="list-decimal space-y-base pl-double">
                  <li>
                    Auth0で上記IdentifierのAPIをRS256で作成し、表示されたスコープを登録します。Resource
                    Parameter Compatibility ProfileとInclude Issuer in
                    Authorization Responsesを有効にします。
                  </li>
                  <li>
                    ChatGPT用のRegular Web
                    ApplicationをWeb用とは別に事前登録し、Authorization
                    Code＋PKCE S256を設定します。Web用Client
                    Secretは流用しません。
                  </li>
                  <li>
                    現在のChatGPT接続画面に表示されるコールバックURLを、そのアプリへ正確に登録します。MCP
                    URLとChatGPT用のClient ID等をChatGPTに入力します。
                  </li>
                  <li>
                    接続して、本人のメンバー承認、読み取り、安全なテストデータへの書き込みを確認します。更新・接続解除の確認は導入手順に従ってください。
                  </li>
                </ol>
                <p>
                  ChatGPT実接続：未検証。上のWebログイン試験とは別の検証です。
                </p>
              </div>
            )}
            {status.authenticated && status.mode === 'auth0' && (
              <div className="space-y-base">
                <h2 className="text-lg text-high">メンバー承認・無効化</h2>
                <p>
                  有効なメンバーは全プロジェクトを利用できます。GoogleとGitHubの本人情報は自動統合しません。
                </p>
                {members.map((member) => (
                  <div
                    className="space-y-half rounded border p-base"
                    key={member.id}
                  >
                    <strong>{member.display_name}</strong>
                    <p className="break-all text-low">
                      {member.issuer} / {member.subject}
                    </p>
                    <p>
                      {member.state} · {member.role}
                    </p>
                    <div className="flex flex-wrap gap-base">
                      {(['active', 'disabled'] as const).map((state) => (
                        <button
                          key={state}
                          className={button}
                          disabled={busy || member.state === state}
                          onClick={() =>
                            void run(async () => {
                              await accessRequest('members', {
                                id: member.id,
                                state,
                                role: member.role,
                              });
                              const result = await accessRequest<{
                                members: Member[];
                              }>('members');
                              setMembers(result.members);
                            })
                          }
                        >
                          {state === 'active' ? '承認・有効化' : '無効化'}
                        </button>
                      ))}
                      <button
                        className={button}
                        disabled={busy || member.state !== 'active'}
                        onClick={() =>
                          void run(async () => {
                            await accessRequest('members', {
                              id: member.id,
                              state: member.state,
                              role:
                                member.role === 'admin' ? 'member' : 'admin',
                            });
                            const result = await accessRequest<{
                              members: Member[];
                            }>('members');
                            setMembers(result.members);
                          })
                        }
                      >
                        {member.role === 'admin'
                          ? '一般メンバーに変更'
                          : '管理者に変更'}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {audit.length > 0 && (
              <details>
                <summary>最近の認証・操作監査</summary>
                <ul className="space-y-half">
                  {audit.map((event) => (
                    <li key={event.id}>
                      {new Date(event.at * 1000).toLocaleString()} ·{' '}
                      {event.actor ?? 'operator / anonymous'} · {event.action} ·{' '}
                      {event.outcome}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            <p className="text-low">
              Webセッションは最長12時間、認証済みHTTPリクエストが30分ないと失効します。LVKログアウトはChatGPTの委任を取り消しません。
            </p>
            {status.authenticated && (
              <a className="block underline" href="/">
                Kanbanへ戻る
              </a>
            )}
          </>
        )}
        {status && !status.limited && !recovery && (
          <a className="block text-low underline" href="/?access=recovery">
            管理者向けの認証設定復旧
          </a>
        )}
      </div>
    </main>
  );
}
